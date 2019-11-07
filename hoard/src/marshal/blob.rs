use std::marker::PhantomData;
use std::mem;
use std::ops;

use super::{Marshal, pile};

#[derive(Debug)]
pub struct Blob<'p, T, Z> {
    marker: PhantomData<(&'p T, fn(Z))>,
    buf: &'p [u8],
}

impl<'p, T: Marshal<Z>, Z: pile::Pile> ops::Deref for Blob<'p, T, Z> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.buf
    }
}

impl<'p, T: Marshal<Z>, Z: pile::Pile> Blob<'p, T, Z> {
    #[inline(always)]
    pub fn new(buf: &'p [u8]) -> Self {
        assert_eq!(buf.len(), T::pile_layout().size());

        Self {
            marker: PhantomData,
            buf,
        }
    }

    #[inline(always)]
    pub unsafe fn assume_valid(self) -> &'p T {
        assert_eq!(self.buf.len(), mem::size_of::<T>());
        assert_eq!(mem::align_of::<T>(), 1);

        &*(self.as_ptr() as *const T)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
