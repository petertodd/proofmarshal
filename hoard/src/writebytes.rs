use std::error;
use std::mem::{self, MaybeUninit};
use std::slice;
use std::ptr;

use thiserror::Error;

use crate::zone::Zone;

pub trait WriteBytes : Sized {
    /// The error returned if a write fails.
    type Error;

    /// Writes a byte slice.
    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;

    /// Writes padding bytes that don't need a specific value.
    ///
    /// The default implementation writes zeros.
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for i in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }
}

impl WriteBytes for Vec<u8> {
    type Error = !;

    #[inline]
    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }

    /// Writes zero-initialized padding bytes.
    #[inline]
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        self.resize(self.len() + len, 0);
        Ok(self)
    }
}

impl WriteBytes for ! {
    type Error = !;

    fn write_bytes(self, _: &[u8]) -> Result<Self, Self::Error> {
        match self {}
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("overflow")]
#[non_exhaustive]
pub struct OverflowError;

/// A `WriteBytes` implementation for uninitialized byte slices.
#[derive(Debug)]
pub struct UninitBytes<'a> {
    written: usize,
    buf: &'a mut [MaybeUninit<u8>],
}

impl<'a> UninitBytes<'a> {
    /// Creates a new `UninitBytes` from a slice.
    #[inline]
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            written: 0,
            buf,
        }
    }

    /*
    /// Splits the the `UninitBytes` into the written and unwritten sub-slices.
    ///
    /// Note that `split_mut()` does *not* exist, because `UninitBytes` can be created from a `&mut
    /// [u8]` slice, and we can't allow that slice to be *un*-initialized.
    pub fn split(self) -> (&'a [u8], &'a [MaybeUninit<u8>]) {
        let (written, remainder) = self.buf.split_at_mut(self.written);

        (unsafe { mem::transmute(written) },
         remainder)
    }
    */

    unsafe fn write_into(
        mut self,
        len: usize,
        f: impl FnOnce(&mut [MaybeUninit<u8>]) -> Result<(), OverflowError>
    ) -> Result<Self, OverflowError>
    {
        let end = self.written.checked_add(len).ok_or(OverflowError)?;
        let dst = self.buf.get_mut(self.written .. end)
                          .ok_or(OverflowError)?;

        debug_assert_eq!(dst.len(), len);
        f(dst)?;

        self.written = end;
        Ok(self)
    }

    /// Returns a fully initialized slice if all bytes have been written.
    #[inline]
    pub fn try_finish(self) -> Result<&'a mut [u8], Self> {
        if self.written < self.buf.len() {
            Err(self)
        } else {
            debug_assert_eq!(self.written, self.buf.len());

            // SAFETY: all bytes written
            unsafe { Ok(mem::transmute(self.buf)) }
        }
    }
}

impl<'a> From<&'a mut [u8]> for UninitBytes<'a> {
    #[inline]
    fn from(slice: &'a mut [u8]) -> Self {
        // SAFETY: UninitBytes only initializes the slice; it will never uninitialize anything.
        let slice: &'a mut [MaybeUninit<u8>] = unsafe { mem::transmute(slice) };
        UninitBytes::new(slice)
    }
}

impl<'a> From<&'a mut [MaybeUninit<u8>]> for UninitBytes<'a> {
    #[inline]
    fn from(slice: &'a mut [MaybeUninit<u8>]) -> Self {
        UninitBytes::new(slice)
    }
}

impl<'a> WriteBytes for UninitBytes<'a> {
    type Error = OverflowError;

    #[inline]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        unsafe {
            self.write_into(src.len(), |dst| {
                // SAFETY: non-overlapping because dst is a &mut
                ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr().cast::<u8>(), src.len());
                Ok(())
            })
        }
    }

    /// Writes zero-initialized padding.
    #[inline]
    fn write_padding(self, len: usize) -> Result<Self, Self::Error> {
        unsafe {
            self.write_into(len, |dst| {
                // SAFETY: valid value for u8
                ptr::write_bytes(dst.as_mut_ptr(), 0, dst.len());
                Ok(())
            })
        }
    }
}

/*
impl<'a, Y: Zone> WriteBlob<Y> for UninitBytes<'a> {
    type Ok = &'a mut [u8];

    fn write<T: SavePoll<Y>>(self, value: &T) -> Result<Self, Self::Error> {
        todo!()
    }

    fn finish(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.try_finish().expect("not all bytes written"))
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninitbytes() {
        let mut buf = [MaybeUninit::<u8>::uninit(); 10];
        let mut w = UninitBytes::new(&mut buf);
        w = w.write_bytes(&[0,1,2,3,4,5,6,7,8,9]).unwrap();
        let buf = w.try_finish().unwrap();
        assert_eq!(buf, &[0,1,2,3,4,5,6,7,8,9]);
    }

    #[test]
    fn uninitbytes_incomplete_finish() {
        let mut buf = [MaybeUninit::<u8>::uninit(); 10];
        let mut w = UninitBytes::new(&mut buf);
        w = w.write_bytes(&[0,1,2,3,4]).unwrap();

        // Incomplete, so we get back the UninitBytes
        w = w.try_finish().unwrap_err();
        assert_eq!(w.written, 5);

        // Finish
        w = w.write_bytes(&[5,6,7,8,9]).unwrap();

        let buf = w.try_finish().unwrap();
        assert_eq!(buf, &[0,1,2,3,4,5,6,7,8,9]);
    }

    #[test]
    fn uninitbytes_overflow() {
        let mut buf = [MaybeUninit::<u8>::uninit(); 0];

        let w = UninitBytes::new(&mut buf);
        assert_eq!(w.write_bytes(&[0]).unwrap_err(), OverflowError);
    }
}
