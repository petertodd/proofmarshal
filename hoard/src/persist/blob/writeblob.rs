use std::convert::TryInto;
use std::io::Cursor;
use std::mem::{self, MaybeUninit};
use std::ptr;

use crate::zone::Zone;
use super::{*, super::encode::Encode};

pub trait WriteBlob : Sized {
    type Ok;
    type Error;

    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;
    fn finish(self) -> Result<Self::Ok, Self::Error>;

    #[inline(always)]
    fn write<'a, Y, T: Encode<'a, Y>>(self, value: &T, state: &T::State) -> Result<Self, Self::Error> {
        value.encode_blob(
            state,
            FieldWriter::new(self, mem::size_of::<T::Encoded>()),
        )
    }

    #[inline(always)]
    fn write_primitive<'a, T: Encode<'a, !>>(self, value: &'a T) -> Result<Self, Self::Error> {
        let state = value.make_encode_state();
        self.write(value, &state)
    }

    /// Writes padding bytes.
    #[inline(always)]
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for i in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }
}

struct FieldWriter<W> {
    inner: W,
    len: usize,
    written: usize,
}

impl<W> FieldWriter<W> {
    fn new(inner: W, len: usize) -> Self {
        Self { inner, len, written: 0 }
    }
}

impl<W: WriteBlob> WriteBlob for FieldWriter<W> {
    type Ok = W;
    type Error = W::Error;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        let written = self.written + src.len();
        assert!(written <= self.len, "overflow");
        Ok(Self {
            len: self.len,
            written,
            inner: self.inner.write_bytes(src)?,
        })
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.written, self.len, "Not all bytes written");
        Ok(self.inner)
    }
}

impl<'a> WriteBlob for Cursor<&'a mut [u8]> {
    type Ok = &'a mut [u8];
    type Error = !;

    #[inline(always)]
    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        let start = self.position().try_into().unwrap();
        let end = start + src.len();

        let dst = self.get_mut().get_mut(start .. end).expect("overflow");
        dst.copy_from_slice(src);

        self.set_position(end.try_into().unwrap());
        Ok(self)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        let pos = self.position().try_into().unwrap();

        let slice = self.into_inner();

        assert_eq!(slice.len(), pos, "Not all bytes written");

        Ok(slice)
    }
}

impl<'a> WriteBlob for Cursor<&'a mut [MaybeUninit<u8>]> {
    type Ok = &'a mut [u8];
    type Error = !;

    #[inline(always)]
    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        let start = self.position().try_into().unwrap();
        let end = start + src.len();

        let dst = self.get_mut().get_mut(start .. end).expect("overflow");

        unsafe {
            assert_eq!(src.len(), dst.len());
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_ptr() as *mut u8, dst.len())
        }

        self.set_position(end.try_into().unwrap());
        Ok(self)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        let pos = self.position().try_into().unwrap();

        let slice = self.into_inner();
        assert_eq!(slice.len(), pos, "Not all bytes written");

        // SAFETY: All bytes have been initialized.
        Ok(unsafe { mem::transmute(slice) })
    }
}

impl<'a> WriteBlob for Vec<u8> {
    type Ok = Self;
    type Error = !;

    #[inline(always)]
    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(src);
        Ok(self)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ptr;

    #[test]
    fn test_cursor_u8() {
        let mut buf = [0;5];
        let dst = Cursor::new(&mut buf[..]);

        let dst = dst.write_bytes(&[]).unwrap();
        assert_eq!(dst.position(), 0);

        let dst = dst.write_bytes(&[1]).unwrap();
        assert_eq!(dst.position(), 1);

        let dst = dst.write_bytes(&[2,3,4,5]).unwrap();
        let dst = dst.finish().unwrap();
        assert!(ptr::eq(dst, &buf[..]));
    }

    #[test]
    fn test_cursor_uninit() {
        let mut buf = [MaybeUninit::<u8>::uninit(); 5];
        let dst = Cursor::new(&mut buf[..]);

        let dst = dst.write_bytes(&[]).unwrap();
        assert_eq!(dst.position(), 0);

        let dst = dst.write_bytes(&[1]).unwrap();
        assert_eq!(dst.position(), 1);

        let dst = dst.write_bytes(&[2,3,4,5]).unwrap();
        let dst = dst.finish().unwrap();
        assert!(ptr::eq(dst, &buf[..] as *const _ as *const [u8]));
    }
}
