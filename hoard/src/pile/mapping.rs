use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::marker::PhantomData;
use std::slice;

#[derive(Debug)]
pub struct Mapping<'a, T: ?Sized> {
    ptr: AtomicPtr<u8>,
    len: AtomicUsize,
    inner: &'a M,
}

impl<'a, T> Mapping<'a, T> {
    pub fn new(inner: &'a T) -> Self
        where M: Borrow<[u8]>
    {
        unsafe {
            Self::new_unchecked(inner.borrow(), inner)
        }
    }

    pub unsafe fn new_unchecked(slice: *const [u8], inner: &'p M) -> Self {
        Self {
            ptr: (slice.as_ptr() as *mut u8).into(),
            len: slice.len().into(),
            inner,
        }
    }
}

impl<'a, T: ?Sized> Mapping<'a, T> {
    pub fn as_bytes(&self) -> &'a [u8] {
        let len = self.len.load(Ordering::Acquire);
        let ptr = self.ptr.load(Ordering::Acquire);

        unsafe {
            slice::from_raw_parts(ptr, len)
        }
    }
}

/*
    pub unsafe fn extend_unchecked<'b>(&'a mut self, new_slice: &'b [u8]) -> &'a Mapping<'b>
        //where 'a: 'b
    {
        let cur_ptr = self.ptr.load(Ordering::Relaxed);
        let cur_len = self.len.load(Ordering::Relaxed);

        self.ptr.compare_exchange(
            cur_ptr,
            new_slice.as_ptr() as *mut u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).expect("exclusive write");

        self.len.compare_exchange(
            cur_len,
            new_slice.len(),
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).expect("exclusive exclusive write");

        self
    }
}
    */

/*
#[derive(Debug)]
pub struct Mapping2<M: ?Sized>(M);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mapping = Mapping2(vec![1u8]);

        let dyn_mapping: &Mapping2<dyn std::fmt::Debug> = &mapping;
    }
}
*/
