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

    fn write<'a, Q, R: ValidateBlob, T: Encode<'a, Q, R>>(self, val: &'a T, state: &T::State) -> Result<Self, Self::Error>
    {
        let dst = Limit::new(self, <T::Encoded as ValidateBlob>::BLOB_LEN);
        val.encode_blob(state, dst)
    }

    fn write_primitive<T: Primitive>(self, val: &T) -> Result<Self, Self::Error> {
        let mut state = Encode::<!,!>::init_encode_state(val);
        val.encode_poll(&mut state, DummySavePtr).into_ok();
        self.write::<!,!,T>(val, &state)
    }
}

struct DummySavePtr;

impl Dumper for DummySavePtr {
    type Source = !;
    type Target = !;
    type Error = !;

    fn save_ptr<'a, T: ?Sized>(self, value: &'a T, state: &T::State) -> Result<(Self, !), !>
        where T: Save<'a, !, !>
    {
        panic!()
    }

    unsafe fn try_save_ptr<'a, T: ?Sized + Pointee>(&mut self, ptr: &'a !, _: T::Metadata) -> Result<!, &'a T> {
        match *ptr {}
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

pub trait SaveBlob {
    type Done;
    type Error : error::Error;

    type WriteBlob : WriteBlob<Done=Self::Done, Error=Self::Error>;
    fn alloc(self, size: usize) -> Result<Self::WriteBlob, Self::Error>;
}

impl SaveBlob for Vec<u8> {
    type Done = Self;
    type Error = !;

    type WriteBlob = Limit<Self>;

    fn alloc(self, size: usize) -> Result<Self::WriteBlob, Self::Error> {
        Ok(Limit::new(self, size))
    }
}
