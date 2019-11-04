use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::mem;
use core::borrow::Borrow;

use std::io::{self, Write, Seek, SeekFrom, BufWriter};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use persist::Persist;

use memmap::Mmap;

mod offset;
pub use self::offset::*;

pub mod disk;
use self::disk::*;

pub struct HoardFile {
    fd: BufWriter<File>,
    mapping: Arc<Mmap>,
}

impl HoardFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut fd = OpenOptions::new()
                        .read(true)
                        .append(true)
                        .open(path)?;
        Self::open_fd(fd)
    }

    pub fn open_fd(mut fd: File) -> io::Result<Self> {
        fd.seek(SeekFrom::End(0))?;

        let mapping = unsafe { Mmap::map(&fd)? };

        if mapping.len() < mem::size_of::<Header>() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "header missing"));
        }
        // FIXME: validate header

        Ok(Self {
            mapping: Arc::new(mapping),
            fd: BufWriter::new(fd),
        })
    }

    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut fd = OpenOptions::new()
                        .read(true)
                        .append(true)
                        .create(true)
                        .open(path)?;

        Self::create_from_fd(fd)
    }

    pub fn create_from_fd(mut fd: File) -> io::Result<Self> {
        let header = Header::default();

        header.write_canonical(&mut fd)?;
        fd.seek(SeekFrom::Start(0))?;
        fd.flush()?;

        Self::open_fd(fd)
    }

    pub fn enter<F,R>(self, f: F) -> R
        where F: for<'h> FnOnce(Hoard<'h>) -> R
    {
        let hoard = Hoard {
            marker: PhantomData,
            mapping: self.mapping,
            fd: self.fd,
            bytes_written: 0,
        };

        f(hoard)
    }
}

#[derive(Debug)]
pub struct Hoard<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    mapping: Arc<Mmap>,
    fd: BufWriter<File>,
    bytes_written: usize,
}

/// Read-only snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    mapping: Arc<Mmap>,
}

impl<'h> Snapshot<'h> {
    fn from_mapping(mapping: Arc<Mmap>) -> Self {
        Self {
            marker: PhantomData,
            mapping,
        }
    }
}

impl<'h> Hoard<'h> {
    fn mapped_bytes(&self) -> &[u8] {
        &self.mapping[mem::size_of::<Header>() - mem::size_of::<Word>() .. ]
    }

    fn write_bytes(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.fd.write(buf) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero,
                                                   "failed to write whole buffer")),
                Ok(n) => {
                    self.bytes_written += n;
                    buf = &buf[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn write_padding(&mut self, len: usize) -> io::Result<()> {
        for _ in 0 .. len {
            self.write_bytes(&[0])?;
        }
        Ok(())
    }

    fn written_offset_bytes(&self) -> usize {
        self.mapped_bytes().len() + self.bytes_written
    }


    pub fn write_blob(&mut self, bytes: &[u8]) -> io::Result<Offset<'h>> {
        if self.written_offset_bytes() % mem::size_of::<Word>() != 0 {
            let padding = mem::size_of::<Word>() - (self.written_offset_bytes() % mem::size_of::<Word>());
            self.write_padding(padding)?;
        }

        assert_eq!(self.written_offset_bytes() % mem::size_of::<Word>(), 0);
        let offset_words = self.written_offset_bytes() % mem::size_of::<Word>();

        let packet = Packet::from(Blob::new(bytes));

        let initial_offset = (self.written_offset_bytes() / mem::size_of::<Word>()) + 1;
        self.write_padding(packet.padding_required(initial_offset as u64))?;

        let offset = self.written_offset_bytes() / mem::size_of::<Word>();

        let mark = u64::max_value() - (offset as u64);
        self.write_bytes(&mark.to_le_bytes())?;

        self.write_bytes(packet.header().as_bytes())?;
        self.write_bytes(bytes)?;

        let offset = Offset::new(offset).expect("non-zero");
        Ok(offset)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.fd.flush()
    }

    pub fn leave(self) -> HoardFile {
        HoardFile {
            fd: self.fd,
            mapping: self.mapping,
        }
    }

    pub fn snapshot(&mut self) -> io::Result<Snapshot<'h>> {
        if self.bytes_written > 0 {
            self.flush()?;
            let mapping = unsafe { Mmap::map(&self.fd.get_ref())? };
            self.bytes_written = 0;
            self.mapping = mapping.into();
        }

        Ok(Snapshot::from_mapping(Arc::clone(&self.mapping)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    use std::io::Read;

    #[test]
    fn test() -> io::Result<()> {
        let mut hoard = HoardFile::create_from_fd(tempfile()?)?;

        assert_eq!(hoard.mapping.len(), mem::size_of::<Header>());

        hoard.enter(|mut hoard| {
            assert_eq!(hoard.mapped_bytes().len(), mem::size_of::<Word>());

            hoard.write_blob(b"hello world!")?;
            hoard.write_blob(b"hello world!")?;

            let snap = hoard.snapshot()?;
            dbg!(&snap.mapping[..]);
            dbg!(hoard);

            Ok(())
        })
        /*


        let mut fd = hoard.into_fd();

        let mut v = vec![];
        fd.read_to_end(&mut v)?;
        dbg!(v);

        Ok(())
        */
    }
}
