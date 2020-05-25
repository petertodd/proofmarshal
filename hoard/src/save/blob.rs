use std::error;
use std::mem;

use crate::save::*;
use crate::pointee::Pointee;
use crate::primitive::Primitive;
use crate::blob::BlobLen;

pub trait WriteBlob : Sized {
    type Done;
    type Error : error::Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;
    fn done(self) -> Result<Self::Done, Self::Error>;

    fn write_padding(mut self, n: usize) -> Result<Self, Self::Error> {
        for i in 0 .. n {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }

    fn write<T: EncodeBlob>(self, val: &T) -> Result<Self, Self::Error>
    {
        let dst = Limit::new(self, T::BLOB_LEN);
        val.encode_blob(dst)
    }

    fn write_primitive<T: Primitive>(self, val: &T) -> Result<Self, Self::Error> {
        let mut encoder = Encode::<!,!>::init_encode(val, &DummySavePtr);
        encoder.save_poll(DummySavePtr).into_ok();
        self.write(&encoder)
    }
}

struct DummySavePtr;

impl SavePtr for DummySavePtr {
    type Source = !;
    type Target = !;
    type Error = !;

    unsafe fn check_dirty<'a, T: ?Sized + Pointee>(&self, ptr: &'a !, _: T::Metadata) -> Result<!, &'a T> {
        match *ptr {}
    }

    fn try_save_ptr(self, _: &impl SaveBlob) -> Result<(Self, Self::Target), Self::Error> {
        panic!()
    }
}


#[derive(Debug)]
pub struct Limit<W> {
    inner: W,
    remaining: usize,
}

impl<W> Limit<W> {
    pub fn new(inner: W, remaining: usize) -> Self {
        Self { inner, remaining }
    }
}

impl<W: WriteBlob> WriteBlob for Limit<W> {
    type Done = W;
    type Error = W::Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self {
            remaining:  self.remaining.checked_sub(buf.len())
                                      .expect("overflow"),
            inner: self.inner.write_bytes(buf)?,
        })
    }

    fn done(self) -> Result<Self::Done, Self::Error> {
        assert_eq!(self.remaining, 0);
        Ok(self.inner)
    }
}

impl WriteBlob for Vec<u8> {
    type Done = Self;
    type Error = !;

    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }

    fn done(self) -> Result<Self::Done, Self::Error> {
        Ok(self)
    }
}

pub trait AllocBlob {
    type Done;
    type Error : error::Error;

    type WriteBlob : WriteBlob<Done=Self::Done, Error=Self::Error>;
    fn alloc_blob(self, size: usize) -> Result<Self::WriteBlob, Self::Error>;
}

impl AllocBlob for Vec<u8> {
    type Done = Self;
    type Error = !;

    type WriteBlob = Limit<Self>;

    fn alloc_blob(self, size: usize) -> Result<Self::WriteBlob, Self::Error> {
        Ok(Limit::new(self, size))
    }
}
