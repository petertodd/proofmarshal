//! Generic borrowed references

use core::ops::Deref;
use core::borrow::Borrow;
use core::fmt;

use crate::ptr::Pointee;

/// A (conceptually) borrowed reference to a value in an arena.
///
/// Implements `Deref<Target=T>`.
///
/// May actually have ownership of the value if the value had to be loaded into memory.
pub struct Ref<'a, T: ?Sized + Pointee>(RefState<'a, T>);

impl<'a, T: ?Sized + Pointee> Ref<'a, T> {
    #[inline(always)]
    pub fn borrowed(r: &'a T) -> Self {
        Ref(RefState::Borrowed(r))
    }

    #[inline(always)]
    pub fn owned(owned: T::Owned) -> Self {
        Ref(RefState::Owned(owned))
    }
}

enum RefState<'a, T: ?Sized + Pointee> {
    Owned(T::Owned),
    Borrowed(&'a T),
}

impl<'p, T: ?Sized + Pointee> Deref for Ref<'p, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match &self.0 {
            RefState::Borrowed(r) => r,
            RefState::Owned(owned) => owned.borrow(),
        }
    }
}

impl<'a, T: ?Sized + Pointee + fmt::Debug> fmt::Debug for Ref<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match &self.0 {
            RefState::Owned(owned) => {
                f.debug_tuple("Ref::Owned")
                    .field(&owned.borrow())
                    .finish()
            },
            RefState::Borrowed(r) => {
                f.debug_tuple("Ref::Borrowed")
                    .field(&r)
                    .finish()
            },
        }
    }
}
