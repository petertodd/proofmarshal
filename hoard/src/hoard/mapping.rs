use core::ops;
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct Mapping<'a,T> {
    start: &'a [T;0],
    len: AtomicUsize,
}

impl<'a, T> Mapping<'a, T> {
    #[inline]
    pub fn new(slice: &'a [T]) -> Self {
        Self {
            start: unsafe { &*slice.as_ptr().cast() },
            len: AtomicUsize::new(slice.len()),
        }
    }

    #[inline]
    pub const fn empty() -> Self {
        Self {
            start: &[],
            len: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub unsafe fn extend_unchecked(&self, additional: usize) {
        self.len.fetch_add(additional, Ordering::AcqRel);
    }
}

impl<T> ops::Deref for Mapping<'_, T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        let len = self.len.load(Ordering::Acquire);
        unsafe {
            slice::from_raw_parts(self.start.as_ptr(), len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;

    #[test]
    fn test() {
        let mut backend = &mut [0u8;100];
        let (used, mut unused) = backend.split_at_mut(0);
        let mapping = &Mapping::new(used);

        let mut write = |src: &[u8]| {
            assert!(src.len() <= unused.len());
            let (dst, rest) = mem::take(&mut unused).split_at_mut(src.len());
            dst.copy_from_slice(src);
            unused = rest;

            unsafe { mapping.extend_unchecked(src.len()) };
        };


        assert_eq!(&**mapping, &[]);

        write(&[1,2,3]);
        assert_eq!(&**mapping, &[1,2,3]);

        write(&[4,5,6]);
        assert_eq!(&**mapping, &[1,2,3,4,5,6]);
    }
}
