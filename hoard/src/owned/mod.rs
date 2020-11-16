//! Traits for working with unsized types.

use core::borrow::Borrow;
use core::ptr;
use core::mem::ManuallyDrop;

pub mod refown;
pub use self::refown::RefOwn;

pub mod take;
pub use self::take::Take;

pub mod refs;
pub use self::refs::Ref;

pub trait IntoOwned {
    type Owned : Borrow<Self> + Take<Self>;
    fn into_owned(self: RefOwn<Self>) -> Self::Owned;
}

impl<T> IntoOwned for T {
    type Owned = Self;

    fn into_owned(self: RefOwn<'_, Self>) -> Self::Owned {
        let this = ManuallyDrop::new(self);

        unsafe {
            ptr::read(&**this)
        }
    }
}

impl<T> IntoOwned for [T] {
    type Owned = Vec<T>;

    fn into_owned(self: RefOwn<'_, Self>) -> Vec<T> {
        let mut r = Vec::with_capacity(self.len());

        let this: &mut [T] = RefOwn::leak(self);

        // SAFETY: since we've leaked the source slice, destructors won't run, making it safe for
        // the Vec to take ownership.
        unsafe {
            ptr::copy_nonoverlapping(this.as_ptr(), r.as_mut_ptr(), this.len());
            r.set_len(this.len());
        }
        r
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn slice_into_owned() {
        // FIXME: actually count # of drops
        let slice = [Box::new(0), Box::new(1), Box::new(2)];
        let mut slice = ManuallyDrop::new(slice);
        let slice: &mut [Box<usize>] = &mut slice[..];

        // SAFETY: can take ownership because of ManuallyDrop
        let slice = unsafe { RefOwn::new_unchecked(slice) };

        let v: Vec<Box<usize>> = slice.into_owned();

        for i in 0 .. v.len() {
            assert_eq!(&*v[i], &i);
        }
    }
}
