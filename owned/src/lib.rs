//! Targets of pointers.

use core::borrow::Borrow;
use core::cell::Cell;
use core::mem::ManuallyDrop;
use core::ptr;

mod take;
pub use self::take::Take;

/// The owned form of a type.
pub unsafe trait IntoOwned {
    type Owned : Borrow<Self> + Take<Self>;

    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned;
}

unsafe impl<T> IntoOwned for T {
    type Owned = T;

    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        (this as *const _ as *const Self).read()
    }
}

unsafe impl<T> IntoOwned for [T] {
    type Owned = Vec<T>;

    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<[T]>) -> Self::Owned {
        let len = this.len();

        let mut r = Vec::<T>::with_capacity(len);

        ptr::copy_nonoverlapping(this.as_ptr(), r.as_mut_ptr(), len);
        r.set_len(len);

        r
    }
}

#[derive(Debug)]
struct CountDrops<'a>(&'a Cell<usize>);

impl Drop for CountDrops<'_> {
    fn drop(&mut self) {
	self.0.set(self.0.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;

    #[test]
    fn count_drops() {
        let drops = Cell::new(0);

        let _ = CountDrops(&drops);
        assert_eq!(drops.get(), 1);

        mem::forget(CountDrops(&drops));
        assert_eq!(drops.get(), 1);
    }
}
