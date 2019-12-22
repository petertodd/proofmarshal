use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use super::*;
use crate::marshal::primitive::Primitive;
use crate::marshal::en::{Encode, SaveState};

pub trait WriteBlob : Sized {
    type Ok;
    type Error : fmt::Debug;

    /// Write an encodable value.
    #[inline(always)]
    fn write_primitive<T: Primitive>(self, value: &T) -> Result<Self, Self::Error> {
        //self.write::<!, T>(value, &())
        todo!()
    }

    /// Write an encodable value.
    #[inline(always)]
    fn write<'a, P: Ptr, T: Encode<P>>(self, value: &'a T, state: &<T as SaveState<'a, P>>::State)
        -> Result<Self, Self::Error>
    {
        /*
        let value_writer = ValueWriter::new(self, T::BLOB_LAYOUT.size());
        value.encode_blob(state, value_writer)
        */ todo!()
    }

    /// Writes bytes to the blob.
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;

    /// Writes padding bytes to the blob.
    #[inline(always)]
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for _ in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }

    /// Finishes writing the blob.
    ///
    /// Will panic if the correct number of bytes hasn't been written.
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

/*
pub(crate) struct ValueWriter<W> {
    inner: W,
    remaining: usize,
}

impl<W> ValueWriter<W> {
    #[inline(always)]
    pub(crate) fn new(inner: W, size: usize) -> Self {
        Self {
            inner,
            remaining: size,
        }
    }
}

impl<W: WriteBlob> WriteBlob for ValueWriter<W> {
    type Ok = W;
    type Error = W::Error;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        let remaining = self.remaining.checked_sub(src.len())
                                      .expect("overflow");
        Ok(Self::new(self.inner.write_bytes(src)?,
                     remaining))
    }

    #[inline(always)]
    fn write_padding(self, len: usize) -> Result<Self, Self::Error> {
        let remaining = self.remaining.checked_sub(len)
                                      .expect("overflow");
        Ok(Self::new(self.inner.write_padding(len)?,
                     remaining))
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.remaining, 0,
                   "not all bytes written");
        Ok(self.inner)
    }
}
*/

impl WriteBlob for &'_ mut [u8] {
    type Ok = ();
    type Error = !;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        if self.len() < src.len() {
            panic!("overflow")
        };

        let (dst, rest) = self.split_at_mut(src.len());
        dst.copy_from_slice(src);
        Ok(rest)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.len(), 0,
                   "not all bytes written");
        Ok(())
    }
}

impl WriteBlob for &'_ mut [MaybeUninit<u8>] {
    type Ok = ();
    type Error = !;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        if self.len() < src.len() {
            panic!("overflow")
        };

        let (dst, rest) = self.split_at_mut(src.len());

        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_ptr() as *mut u8, src.len());
        }

        Ok(rest)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.len(), 0,
                   "not all bytes written");
        Ok(())
    }
}

/*
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_exact_u8_slice() -> Result<(), !> {
        let mut buf = [0,0,0];

        let w = &mut buf[..];
        w.write_bytes(&[1])?
         .write_bytes(&[2])?
         .write_bytes(&[3])?
         .finish()?;

        assert_eq!(buf, [1,2,3]);

        Ok(())
    }
}
*/
