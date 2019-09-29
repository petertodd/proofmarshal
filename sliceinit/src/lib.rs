use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ptr;
use core::slice;

pub struct SliceInitializer<'a, T> {
    marker: PhantomData<&'a mut T>,
    uninit: &'a mut [MaybeUninit<T>],
    written: usize,
}

impl<'a, T> Drop for SliceInitializer<'a,T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.as_init_mut())
        }
    }
}

impl<'a, T> SliceInitializer<'a,T> {
    #[inline(always)]
    pub fn new(uninit: &'a mut [MaybeUninit<T>]) -> Self {
        Self {
            marker: PhantomData,
            uninit,
            written: 0,
        }
    }

    #[inline(always)]
    pub fn as_init(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.uninit.as_ptr() as *const T, self.written)
        }
    }

    #[inline(always)]
    pub fn as_init_mut(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.uninit.as_mut_ptr() as *mut T, self.written)
        }
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        let uninit_item = &mut self.uninit[self.written];

        unsafe {
            uninit_item.as_mut_ptr().write(value)
        }
        self.written += 1;
    }

    #[inline]
    pub fn done(self) -> &'a mut [T] {
        assert_eq!(self.uninit.len(), self.written,
                   "slice not fully initialized");

        let mut this = ManuallyDrop::new(self);
        unsafe {
            slice::from_raw_parts_mut(this.uninit.as_mut_ptr() as *mut T,
                                      this.written)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::mem;
    use std::cell::Cell;

    struct CheckDrop<'a>(&'a Cell<usize>);

    impl Drop for CheckDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn test_checkdrop() {
        let num_drops = Cell::new(0);

        let checkdrop = CheckDrop(&num_drops);
        assert_eq!(num_drops.get(), 0);

        mem::drop(checkdrop);
        assert_eq!(num_drops.get(), 1);
    }

    #[test]
    fn test() {
        let drops: [Cell<usize>; 4] = Default::default();

        let mut uninit: [MaybeUninit<CheckDrop>; 4] = unsafe { MaybeUninit::uninit().assume_init() };

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
