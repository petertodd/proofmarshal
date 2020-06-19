use std::error;
use std::mem::{self, MaybeUninit};
use std::slice;

use thiserror::Error;

pub trait WriteBytes : Sized {
    type Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;
    fn write_padding(mut self, n: usize) -> Result<Self, Self::Error> {
        for i in 0 .. n {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }
}

impl WriteBytes for Vec<u8> {
    type Error = !;

    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }

    fn write_padding(mut self, n: usize) -> Result<Self, Self::Error> {
        self.resize(self.len() + n, 0);
        Ok(self)
    }
}

impl WriteBytes for ! {
    type Error = !;

    fn write_bytes(self, _: &[u8]) -> Result<Self, Self::Error> {
        match self {}
    }
}

#[derive(Debug, Error)]
#[error("overflow")]
#[non_exhaustive]
pub struct OverflowError;

#[derive(Debug)]
pub struct UninitBytes<'a> {
    written: usize,
    buf: &'a mut [MaybeUninit<u8>],
}

impl<'a> UninitBytes<'a> {
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            written: 0,
            buf,
        }
    }

    pub fn split(self) -> (&'a [u8], &'a [MaybeUninit<u8>]) {
        let (written, remainder) = self.buf.split_at_mut(self.written);

        (unsafe { mem::transmute(written) },
         remainder)
    }
}

/*
impl<'a> Write for UninitBytes<&'a mut [u8]> {
    type Error = OverflowError;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let end = self.written.checked_add(buf.len()).ok_or(OverflowError)?;
        let mut dst = self.buf.get_mut(self.written .. end)
                              .ok_or(OverflowError)?;

        dst.copy_from_slice(buf);

        self.written = end;
        Ok(())
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor() {
    }
}
