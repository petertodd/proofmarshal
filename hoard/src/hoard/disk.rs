use std::convert::{TryFrom, TryInto};
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
    const MAGIC: [u8; 16] = [76; 16];
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
    pub fn new(offset_words: u64) -> Self {
        Self((u64::max_value() - offset_words).into())
    }

    pub fn is_valid(&self, offset: u64) -> bool {
        *self == Self::new(offset)
    }

    pub fn as_marks(bytes: &[u8]) -> &[Mark] {
        unsafe {
            let (prefix, marks, _padding) = bytes.align_to();
            assert_eq!(prefix.len(), 0);
            marks
        }
    }

    pub fn as_bytes(&self) -> &[u8; size_of::<Self>()] {
        unsafe {
            &*(self as *const _ as *const _)
        }
    }
}

#[derive(Debug)]
pub struct BlobDumper<'f, 'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    fd: &'f mut File,
    written: Option<u64>,
    pending: Vec<u8>,
}

impl<'f, 'h> BlobDumper<'f, 'h> {
    pub fn new(fd: &'f mut File) -> io::Result<Self> {
        Self::with_capacity(8192, fd)
    }

    pub fn with_capacity(capacity: usize, fd: &'f mut File) -> io::Result<Self> {
        let written = fd.seek(SeekFrom::End(0))?
                        .checked_sub(size_of::<FileHeader>() as u64)
                        .expect("missing header");

        Ok(Self {
            marker: PhantomData,
            written: Some(written),
            pending: Vec::with_capacity(capacity),
            fd,
        })
    }

    pub fn written(&self) -> io::Result<u64> {
        self.written.ok_or_else(|| io::Error::new(io::ErrorKind::Other, "previously failed"))
    }

    pub fn flush_pending(&mut self) -> io::Result<()> {
        let written = self.written.take().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "previously failed"))?;

        self.fd.write_all(&self.pending)?;
        let written = written + self.pending.len() as u64;
        self.pending.clear();

        let actual_pos = self.fd.seek(SeekFrom::Current(0))?;
        let expected_pos = size_of::<FileHeader>() as u64 + written;
        if actual_pos == expected_pos as u64 {
            self.written = Some(written);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, format!("file position {} != expected position {}",
                                                             actual_pos, expected_pos)))
        }
    }

    pub fn write_blob_with(&mut self, size: usize, f: impl FnOnce(&mut [u8])) -> io::Result<u64> {
        // Note how one big write will increase the capacity forever after!
        if self.pending.len() + size > self.pending.capacity() {
            self.flush_pending()?;
        }

        let written = self.written()?;

        let start = self.pending.len();
        let offset = written + start as u64;

        let end = offset + size as u64;
        let padding = usize::try_from(round_up(end, size_of::<Mark>()) - end).unwrap();

        self.pending.resize(start + size + padding, 0);

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
        Ok(written + start as u64)
    }

    pub fn write_blob(&mut self, blob: impl AsRef<[u8]>) -> io::Result<u64> {
        let blob = blob.as_ref();
        self.write_blob_with(blob.len(), |dst| dst.copy_from_slice(blob))
    }

    pub fn write_padding(&mut self, align: usize) -> io::Result<()> {
        let written = self.written()?;
        let padding = align_offset(written, align);

        self.pending.resize(self.pending.len() + padding, 0);
        self.written = Some(written + padding as u64);
        Ok(())
    }

    pub fn commit_root_with(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> io::Result<u64> {
        // Start the root blob on a mark boundry..
        self.write_padding(size_of::<Mark>())?;

        self.write_blob_with(size, f)?;

        // ...and align the end to a mark.
        self.write_padding(size_of::<Mark>())?;

        self.flush_pending()?;

        let offset_bytes = self.written()?;
        assert_eq!(offset_bytes % size_of::<Mark>() as u64, 0);
        let offset_marks = offset_bytes / size_of::<Mark>() as u64;
        let mark = Mark::new(offset_marks);
        self.fd.write_all(mark.as_bytes())?;
        self.fd.flush()?;

        Ok(offset_bytes)
    }
}

fn round_up(n: u64, align: usize) -> u64 {
    assert!(align.is_power_of_two());
    let align = u64::try_from(align).unwrap();
    (n + align - 1) / align * align
}

pub fn align_offset(offset: u64, align: usize) -> usize {
    assert!(align.is_power_of_two());
    usize::try_from(round_up(offset, align) - offset).unwrap()
}

fn calc_padding_bytes_required(offset: u64, buf: &[u8]) -> usize {
    0
    /*
    assert_eq!(buf.len() % size_of::<Mark>(), 0);
    assert_eq!(offset % size_of::<Mark>() as u64, 0);

    let offset_words = offset / size_of::<Mark>() as u64;
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
    */
}

#[cfg(test)]
mod tests {
    use super::*;
}
