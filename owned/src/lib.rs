//! Targets of pointers.

use core::borrow::Borrow;
use core::cell::Cell;
use core::mem::ManuallyDrop;
use core::ptr;
use core::slice;

mod take;
pub use self::take::Take;

mod refs;
pub use self::refs::Ref;

/// The owned form of a type.
pub unsafe trait Owned {
    type Owned : Borrow<Self>;

    unsafe fn to_owned(this: &mut ManuallyDrop<Self>) -> Self::Owned;

    unsafe fn from_owned<'a>(owned: Self::Owned, dst: *mut ()) -> &'a mut Self;
}

unsafe impl<T> Owned for T {
    type Owned = T;

    unsafe fn to_owned(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        (this as *const _ as *const Self).read()
    }

    unsafe fn from_owned<'a>(owned: Self::Owned, dst: *mut ()) -> &'a mut Self {
        let dst = dst.cast::<Self>();
        dst.write(owned);
        &mut *dst
    }
}

unsafe impl<T> Owned for [T] {
    type Owned = Vec<T>;

    unsafe fn to_owned(this: &mut ManuallyDrop<[T]>) -> Self::Owned {
        let len = this.len();

        let mut r = Vec::<T>::with_capacity(len);

        ptr::copy_nonoverlapping(this.as_ptr(), r.as_mut_ptr(), len);
        r.set_len(len);

        r
    }

    unsafe fn from_owned<'a>(mut owned: Vec<T>, dst: *mut ()) -> &'a mut Self {
        let dst = dst as *mut T;
        let len = owned.len();

        owned.set_len(0);
        ptr::copy_nonoverlapping(owned.as_ptr(), dst as *mut T, len);

        slice::from_raw_parts_mut(dst, len)
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
