//! In-place allocation.

use std::mem::{self, MaybeUninit};
use std::ptr;
use std::slice;

use crate::pointee::Pointee;

/// A place where you can put an (unsized) value into.
pub trait Emplace<T: ?Sized + Pointee> : Sized {
    type Done;

    /// Emplace an unsized value.
    unsafe fn emplace_unsized(self, metadata: T::Metadata, f: impl FnOnce(*mut T)) -> Self::Done;

    /// Emplace a sized value.
    fn emplace(self, value: T) -> Self::Done
        where T: Sized,
    {
        unsafe {
            self.emplace_unsized(value.ptr_metadata(),
                                |ptr| ptr.write(value))
        }
    }
}


pub struct SliceInitializer<'a, T> {
    uninit: &'a mut [MaybeUninit<T>],
    written: usize,
}

impl<T> Drop for SliceInitializer<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.as_init_mut())
        }
    }
}

impl<'a, T> SliceInitializer<'a, T> {
    #[inline]
    pub fn new(uninit: &'a mut [MaybeUninit<T>]) -> Self {
        SliceInitializer {
            uninit,
            written: 0,
        }
    }


    #[inline]
    pub fn as_init(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.uninit.as_ptr() as *const T, self.written)
        }
    }

    #[inline]
    pub fn as_init_mut(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.uninit.as_mut_ptr() as *mut T, self.written)
        }
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        self.uninit[self.written].write(value);
        self.written += 1;
    }

    #[inline]
    pub fn done(self) -> &'a mut [T] {
        assert_eq!(self.uninit.len(), self.written,
                   "slice not fully initialized");

        unsafe {
            let r = slice::from_raw_parts_mut(self.uninit.as_mut_ptr() as *mut T, self.uninit.len());
            mem::forget(self);
            r
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::cell::Cell;

    struct CheckDrop<'a>(&'a Cell<usize>);

    impl Drop for CheckDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn test() {
        let drops: [Cell<usize>; 4] = Default::default();

        let uninit = MaybeUninit::<[CheckDrop; 4]>::uninit();
        let mut uninit: [MaybeUninit<CheckDrop>; 4] = unsafe { mem::transmute(uninit) };

        {
            let mut initializer = SliceInitializer::new(&mut uninit);
            initializer.push(CheckDrop(&drops[0]));
        }
        assert_eq!(drops[0].get(), 1);

        let _ = SliceInitializer::new(&mut uninit);
        assert_eq!(drops[0].get(), 1);

        {
            let mut initializer = SliceInitializer::new(&mut uninit);
            initializer.push(CheckDrop(&drops[0]));
            initializer.push(CheckDrop(&drops[1]));
        }
        assert_eq!((drops[0].get(), drops[1].get(), drops[2].get(), drops[3].get()),
                   (2,1,0,0));

        {
            let mut initializer = SliceInitializer::new(&mut uninit);
            initializer.push(CheckDrop(&drops[0]));
            initializer.push(CheckDrop(&drops[1]));
            initializer.push(CheckDrop(&drops[2]));
            initializer.push(CheckDrop(&drops[3]));

            assert_eq!((drops[0].get(), drops[1].get(), drops[2].get(), drops[3].get()),
                       (2,1,0,0));

            let init: &[CheckDrop] = initializer.done();
            assert_eq!((drops[0].get(), drops[1].get(), drops[2].get(), drops[3].get()),
                       (2,1,0,0));

            assert_eq!(init[0].0.get(), 2);
        }

        // *slice* went of of scope, so Drop not run
        assert_eq!((drops[0].get(), drops[1].get(), drops[2].get(), drops[3].get()),
                   (2,1,0,0));
    }
}
