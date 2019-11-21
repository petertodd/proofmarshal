use std::convert::TryFrom;
use std::marker::PhantomData;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::fmt;
use std::hash::Hash;
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::mem::{self, size_of};
use std::ops;
use std::slice;
use std::sync::Arc;

use memmap::Mmap;

use leint::Le;

use super::Hoard;

pub trait Flavor : 'static + fmt::Debug + Send + Sync {
    const MAGIC: [u8; 16];
    const MIN_VERSION: u16;
    const MAX_VERSION: u16;
}

impl Flavor for () {
    const MAGIC: [u8; 16] = [0; 16];
    const MIN_VERSION: u16 = 0;
    const MAX_VERSION: u16 = 0;
}

const MAGIC: [u8;12] = *b"\x00Hoard File\x00";

#[repr(C)]
#[derive(Debug)]
pub struct FileHeader<V=()> {
    marker: PhantomData<fn(V)>,
    pub magic: [u8;12],
    pub version: Le<u16>,
    pub flavor_version: Le<u16>,
    pub flavor_magic: [u8;16],
}

impl<V: Flavor> Default for FileHeader<V> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
            magic: MAGIC,
            version: 0.into(),
            flavor_magic: V::MAGIC,
            flavor_version: V::MAX_VERSION.into(),
        }
    }
}

impl<V> FileHeader<V> {
    pub fn as_bytes(&self) -> &[u8; size_of::<FileHeader>()] {
        unsafe {
            &*(self as *const _ as *const _)
        }
    }

    pub fn read(mut fd: impl Read) -> io::Result<Self> {
        let mut buf = [0u8; size_of::<FileHeader>()];

        fd.read_exact(&mut buf)?;

        let this: Self = unsafe { mem::transmute(buf) };

        // TODO: where should we validate magic/version exactly?
        Ok(this)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Mark(Le<u64>);

impl Mark {
    pub fn new(mark_offset: usize) -> Self {
        Self((u64::max_value() - mark_offset as u64).into())
    }

    pub fn is_valid(&self, mark_offset: usize) -> bool {
        *self == Self::new(mark_offset)
    }
}


#[derive(Debug, Clone)]
pub struct Mapping {
    mapping: Arc<dyn fmt::Debug + Send + Sync>,
    slice: &'static [u8],
}

impl Mapping {
    pub fn from_file(fd: &File) -> io::Result<Self> {
        let mapping = unsafe { Mmap::map(&fd)? };

        let slice = mapping.get(size_of::<FileHeader>() .. )
                           .expect("missing header");

        Ok(Self {
            slice: unsafe { mem::transmute(slice) },
            mapping: Arc::new(mapping),
        })
    }

    pub fn from_buf<B>(buf: B) -> Self
        where B: 'static + AsRef<[u8]> + fmt::Debug + Send + Sync,
    {
        // Important to do this first so as to pin down the buffer's memory location.
        let buf = Arc::new(buf);

        // Remember that as_ref() doesn't necessarily have to return the same slice each time, so
        // we have to be careful to call as_ref() exactly once.
        let slice = (*buf).as_ref();
        Self {
            slice: unsafe { mem::transmute(slice) },
            mapping: buf,
        }
    }

    pub fn as_marks(&self) -> &[Mark] {
        unsafe {
            let (prefix, marks, _padding) = self.align_to();
            assert_eq!(prefix.len(), 0);
            marks
        }
    }

    pub fn truncate(&mut self, len: usize) {
        if let Some(slice) = self.slice.get(0 .. len) {
            self.slice = slice;
        }
    }

    pub fn slice(&self) -> &&[u8] {
        &self.slice
    }
}

impl ops::Deref for Mapping {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.slice()
    }
}

/*
#[derive(Debug)]
pub struct HoardFile {
    fd: File,
    pub(super) mapping: Mapping,
}

impl HoardFile {
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let fd = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path)?;
        Self::create_from_fd(fd)
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let fd = OpenOptions::new()
            .append(true)
            .open(path)?;
        Self::open_fd(fd)
    }

    pub fn open_fd(mut fd: File) -> io::Result<Self> {
        // FIXME: validate header
        fd.seek(SeekFrom::End(0))?;
        Ok(HoardFile {
            mapping: Mapping::from_file(&fd)?,
            fd,
        })
    }

    pub fn create_from_fd(mut fd: File) -> io::Result<Self> {
        let header_offset = fd.seek(SeekFrom::Current(0))?;
        assert_eq!(header_offset, 0);

        let header = FileHeader::default();
        fd.write_all(header.as_bytes())?;
        fd.flush()?;
        fd.seek(SeekFrom::Start(header_offset))?;

        Ok(HoardFile {
            mapping: Mapping::from_file(&fd)?,
            fd,
        })
    }

    pub fn enter<R>(&mut self, f: impl FnOnce(&mut Hoard) -> R) -> R {
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
        self.hoard.mapping = Mapping::from_file(&self.hoard.fd)?;
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
*/
