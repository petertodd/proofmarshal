use core::borrow::Borrow;
use core::cmp;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU64;
use core::slice;

use std::sync::Arc;

use persist::Persist;

use memmap::Mmap;

use crate::*;

pub mod mapping;
use self::mapping::Mapping;

mod offset;
pub use self::offset::*;

/*
mod snapshot;
pub use self::snapshot::*;

pub mod disk;
use self::disk::*;
*/

#[derive(Debug)]
pub struct VirtualHoard {
    buf: Vec<u8>,
}

impl VirtualHoard {
    pub fn new(capacity: usize) -> Self {
        Self { buf: Vec::with_capacity(capacity) }
    }

    pub fn enter<R>(&mut self, f: impl for<'h> FnOnce(Hoard<'h, Self>) -> R) -> R {
        let buf = unsafe { slice::from_raw_parts(self.buf.as_ptr(), 0) };
        let mapping = Mapping::new(buf);

        let hoard = Hoard {
            backend: self,
            mapping: &mapping,
        };
        f(hoard)
    }
}

#[derive(Debug)]
pub struct Hoard<'h, Backend> {
    backend: &'h mut Backend,
    mapping: &'h Mapping<'h, u8>,
}

impl<'h, Backend> Hoard<'h, Backend> {
    pub fn pile(&self) -> Pile<'h> {
        unsafe {
            Pile::from_mapping(self.mapping)
        }
    }
}

impl<'h> Hoard<'h, VirtualHoard> {
    pub fn write_bytes(&mut self, buf: &[u8]) -> usize {
        let start = self.mapping.len();
        let remaining = self.backend.buf.capacity() - self.mapping.len();
        let mut remaining = unsafe {
            slice::from_raw_parts_mut(self.backend.buf.as_mut_ptr().offset(start as isize),
                                      remaining)
        };

        assert!(buf.len() <= remaining.len(), "out of capacity");
        let (dst, _) = remaining.split_at_mut(buf.len());
        dst.copy_from_slice(buf);

        unsafe {
            self.mapping.extend_unchecked(dst.len());
        }

        start
    }

    pub fn write_blob(&mut self, bytes: &[u8]) -> Offset<'h> {
        /*
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
        */
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Pile<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    mapping: &'h Mapping<'h, u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PileMut<'h>(Pile<'h>);

impl<'h> Pile<'h> {
    unsafe fn from_mapping(mapping: &'h Mapping<u8>) -> Self {
        Self {
            marker: PhantomData,
            mapping,
        }
    }
}

impl<'h> PileMut<'h> {
    unsafe fn from_mapping(mapping: &'h Mapping<u8>) -> Self {
        Self(Pile::from_mapping(mapping))
    }
}


impl Default for PileMut<'_> {
    fn default() -> Self {
        static EMPTY: Mapping<'static, u8> = Mapping::empty();
        unsafe {
            Self::from_mapping(&EMPTY)
        }
    }
}

impl<'h> Zone for Pile<'h> {
    type Ptr = Offset<'h>;
    type Allocator = crate::never::NeverAlloc<Self>;
    type Error = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(_ptr: Ptr<T, Self>) {}
}

impl<'h> Zone for PileMut<'h> {
    type Ptr = OffsetMut<'h>;
    type Allocator = Self;
    type Error = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(_ptr: Ptr<T, Self>) {
        unimplemented!()
    }
}

impl<'h> Alloc for PileMut<'h> {
    type Zone = Self;

    fn alloc<T: Store<Self::Zone>>(&mut self, value: T) -> Rec<T, Self::Zone> {
        unimplemented!()
    }

    fn zone(&self) -> Self {
        self.clone()
    }
}

impl cmp::PartialEq for Pile<'_> {
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}
impl cmp::Eq for Pile<'_> {}

impl<'h> Get for Pile<'_> {
    fn get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T, Self>) -> Ref<'p, T, Self> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    use std::io::Read;

    #[test]
    fn test() {
        VirtualHoard::new(1_000_000).enter(|mut hoard| {
            let p1 = hoard.pile();
            let p2 = hoard.pile();

            dbg!(p2);

            hoard.write_bytes(&[1,2,3]);
            dbg!(p2);
            hoard.write_bytes(&[1,2,3]);
            dbg!(p1);
        });
    }
}

/*
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

*/
