use super::IntoOwned;

use core::mem::{self, ManuallyDrop};
use core::slice;

/// A trait for taking data.
///
/// Implementing `Take<T>` is like implementing `Borrow<T>` but for transferring ownership.
///
/// # Safety
///
/// This trait is unsafe to implement because `take_unsized()` must not `drop()` or otherwise use
/// the taken value after the closure returns.
pub unsafe trait Take<T: ?Sized> : Sized {
    /// Takes ownership of `Sized` type.
    fn take_sized(self) -> T
        where T: Sized
    {
        self.take_unsized(|src| unsafe {
            (src as *const _ as *const T).read()
        })
    }

    /// Takes ownership of the owned version of an unsized type.
    fn take_owned(self) -> T::Owned
        where T: IntoOwned
    {
        self.take_unsized(|src| unsafe { T::into_owned_unchecked(src) })
    }

    /// Takes ownership of an unsized type with the aid of a closure.
    ///
    /// The closure is called with an immutable reference to `ManuallyDrop<T>`. After the closure
    /// returns the memory occupied by the value will be deallocated, but `drop()` will *not* be
    /// called on the value itself.
    ///
    /// `take_sized()` and `take_owned()` are implemented in terms of this.
    fn take_unsized<F,R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R;
}

unsafe impl<T> Take<T> for T {
    fn take_unsized<F,R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        f(&mut this)
    }
}

unsafe impl<T> Take<T> for ManuallyDrop<T> {
    fn take_unsized<F,R>(mut self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R
    {
        f(&mut self)
    }
}

unsafe impl<T: ?Sized + IntoOwned> Take<T> for Box<T> {
    fn take_unsized<F,R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R
    {
        let ptr = Box::into_raw(self) as *mut ManuallyDrop<T>;

        unsafe {
            let mut this: Box<ManuallyDrop<T>> = Box::from_raw(ptr);
            f(&mut this)
        }
    }
}

unsafe impl<T> Take<[T]> for Vec<T> {
    fn take_unsized<F,R>(mut self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<[T]>) -> R
    {
        unsafe {
            let len = self.len();
            self.set_len(0);
            let src: &mut [T] = slice::from_raw_parts_mut(self.as_mut_ptr(), len);
            f(mem::transmute(src))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::CountDrops;

    use std::cell::Cell;

    #[test]
    fn sized() {
        let drops = Cell::new(0);
        let checker = CountDrops(&drops);
        let checker = checker.take_sized();
        assert_eq!(drops.get(), 0);
        drop(checker);
        assert_eq!(drops.get(), 1);

        let drops = Cell::new(0);
        let checker = CountDrops(&drops);
        let checker = checker.take_sized();
        assert_eq!(drops.get(), 0);

        drop(checker);
        assert_eq!(drops.get(), 1);

        let drops = Cell::new(0);
        {
            let checker = CountDrops(&drops);
            checker.take_unsized(|_| {});
        }
        assert_eq!(drops.get(), 0);
    }

    #[test]
    fn boxed() {
        let drops = Cell::new(0);

        let checker = Box::new(CountDrops(&drops));
        let checker: CountDrops = checker.take_sized();
        assert_eq!(drops.get(), 0);

        drop(checker);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn boxed_slice() {
        let drops = Cell::new(0);
        let boxed = vec![CountDrops(&drops)].into_boxed_slice();
        assert_eq!(drops.get(), 0);

        drop(boxed);
        assert_eq!(drops.get(), 1);

        let drops = Cell::new(0);
        let boxed = vec![CountDrops(&drops)].into_boxed_slice();

        boxed.take_unsized(|_: &mut ManuallyDrop<[CountDrops]>| {
        });
        assert_eq!(drops.get(), 0);

        let drops = Cell::new(0);
        let boxed = vec![CountDrops(&drops)].into_boxed_slice();

        let v: Vec<CountDrops> = Take::<[CountDrops]>::take_owned(boxed);
        assert_eq!(drops.get(), 0);
        drop(v);
        assert_eq!(drops.get(), 1);
    }
}
