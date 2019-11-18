use std::collections::hash_map::DefaultHasher;
use std::convert::TryFrom;
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Write, Seek, SeekFrom};
use std::mem::size_of;
use std::ops;
use std::slice;
use std::sync::Arc;

use memmap::Mmap;

use super::Hoard;

#[repr(C)]
#[derive(Default, Debug)]
pub struct FileHeader {
    magic: [u8;16],
}

impl FileHeader {
    pub fn as_bytes(&self) -> &[u8; size_of::<Self>()] {
        unsafe { &*(self as *const _ as *const _) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Mark(u64);

#[derive(Debug, Clone)]
pub struct Mapping {
    mmap: Arc<Mmap>,
    mapping_ptr: *const u8,
    mapping_len: usize,
}

impl Mapping {
    fn new(fd: &File) -> io::Result<Self> {
        let mmap = unsafe { Mmap::map(&fd)? };
        assert!(mmap.len() >= size_of::<FileHeader>());

        let mapping = mmap.get(size_of::<FileHeader>() .. )
                          .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing header"))?;

        Ok(Self {
            mapping_ptr: mapping.as_ptr(),
            mapping_len: mapping.len(),
            mmap: Arc::new(mmap),
        })
    }

    pub fn mark_offsets(&self) -> impl DoubleEndedIterator<Item=usize> + '_ {
        self.chunks_exact(size_of::<Mark>()).enumerate()
            .filter_map(|(offset_words, chunk)| {
                let chunk = <[u8; size_of::<Mark>()]>::try_from(chunk).unwrap();
                let potential_mark = u64::from_le_bytes(chunk);

                if offset_words_to_mark(offset_words) == potential_mark {
                    Some(offset_words * size_of::<u64>())
                } else {
                    None
                }
            })
    }
}

impl ops::Deref for Mapping {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.mapping_ptr, self.mapping_len) }
    }
}

#[derive(Debug)]
pub struct HoardFile {
    fd: File,
    pub(super) mapping: Mapping,

}

impl HoardFile {
    pub fn create_from_fd(mut fd: File) -> io::Result<Self> {
        let header_offset = fd.seek(SeekFrom::Current(0))?;
        assert_eq!(header_offset, 0);

        let header = FileHeader::default();
        fd.write_all(header.as_bytes())?;
        fd.flush()?;
        fd.seek(SeekFrom::Start(header_offset))?;

        Ok(HoardFile {
            mapping: Mapping::new(&fd)?,
            fd,
        })
    }

    pub fn enter<R>(&mut self, f: impl for<'f> FnOnce(Hoard<'f>) -> R) -> R {
        unsafe {
            let hoard = Hoard::new_unchecked(self);
            f(hoard)
        }
    }
}

#[derive(Debug)]
pub struct Tx<'f> {
    hoard: &'f mut HoardFile,
    written: Option<usize>,
    pending: Vec<u8>,
}

const DEFAULT_TX_CAPACITY: usize = 8192;

impl<'f> Tx<'f> {
    pub fn new(hoard: &'f mut HoardFile) -> io::Result<Self> {
        Self::with_capacity(DEFAULT_TX_CAPACITY, hoard)
    }

    pub fn with_capacity(capacity: usize, hoard: &'f mut HoardFile) -> io::Result<Self> {
        hoard.fd.seek(SeekFrom::End(0))?;

        let pending = Vec::with_capacity(capacity);
        if hoard.mapping.len() % size_of::<Mark>() != 0 {
            unimplemented!("partial write")
        };

        Ok(Self {
            written: Some(hoard.mapping.len()),
            pending,
            hoard,
        })
    }

    pub fn flush_pending(&mut self) -> io::Result<()> {
        let written = self.written.take().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "previously failed"))?;

        self.hoard.fd.write_all(&self.pending)?;
        let written = written + self.pending.len();
        self.pending.clear();

        let actual_pos = self.hoard.fd.seek(SeekFrom::Current(0))?;
        let expected_pos = size_of::<FileHeader>() + written;
        if actual_pos == expected_pos as u64 {
            self.written = Some(written);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, format!("file position {} != expected position {}",
                                                             actual_pos, expected_pos)))
        }
    }

    pub fn write_blob_with(&mut self, size: usize, f: impl FnOnce(&mut [u8])) -> io::Result<usize> {
        // Note how one big write will increase the capacity forever after!
        if self.pending.len() + size > self.pending.capacity() {
            self.flush_pending()?;
        }

        let written = self.written.ok_or_else(|| io::Error::new(io::ErrorKind::Other, "previously failed"))?;

        let start = self.pending.len();
        let offset = written + start;

        let end = offset + size;
        let padding = round_up(end, size_of::<Mark>()) - end;

        self.pending.resize_with(start + size + padding, u8::default);

        let dst = &mut self.pending[start .. start + size];
        f(dst);


        let start = match calc_padding_bytes_required(offset, &self.pending[start ..]) {
            0 => start,
            padding_required => {
                self.pending.resize_with(self.pending.len() + padding_required, u8::default);
                self.pending.copy_within(start .. start + size, start + padding_required);
                start + padding_required
            }
        };
        Ok(written + start)
    }

    pub fn write_blob(&mut self, blob: impl AsRef<[u8]>) -> io::Result<usize> {
        let blob = blob.as_ref();
        self.write_blob_with(blob.len(), |dst| dst.copy_from_slice(blob))
    }

    pub fn commit(mut self) -> io::Result<()> {
        self.flush_pending()?;
        let mark = offset_bytes_to_mark(self.written.unwrap());
        self.hoard.fd.write_all(&mark.to_le_bytes())?;
        self.hoard.fd.flush()?;
        self.hoard.mapping = Mapping::new(&self.hoard.fd)?;
        Ok(())
    }
}

fn round_up(n: usize, align: usize) -> usize {
    (n + align - 1) / align * align
}

fn offset_bytes_to_mark(offset: usize) -> u64 {
    assert_eq!(offset % size_of::<Mark>(), 0);
    offset_words_to_mark(offset / size_of::<Mark>())
}

fn offset_words_to_mark(offset: usize) -> u64 {
    u64::max_value() - offset as u64
}

fn calc_padding_bytes_required(offset: usize, buf: &[u8]) -> usize {
    assert_eq!(buf.len() % size_of::<Mark>(), 0);
    assert_eq!(offset % size_of::<Mark>(), 0);

    let offset_words = offset / size_of::<Mark>();
    let len_words = buf.len() / size_of::<Mark>();

    'outer: for padding_words in 0 ..= len_words {
        for (i, chunk) in buf.chunks_exact(size_of::<Mark>()).enumerate() {
            let chunk = <[u8;size_of::<u64>()]>::try_from(chunk).unwrap();
            let potential_mark = u64::from_le_bytes(chunk);

            if offset_words_to_mark(offset_words + padding_words) == potential_mark {
                break 'outer;
            }
        }
        return padding_words * size_of::<Mark>();
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
}
