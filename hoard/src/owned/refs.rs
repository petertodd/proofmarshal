use std::ops::Deref;
use std::borrow::Borrow;

use super::IntoOwned;

#[derive(Debug)]
pub enum Ref<'a, T: ?Sized + IntoOwned> {
    Borrowed(&'a T),
    Owned(T::Owned),
}

impl<T: ?Sized + IntoOwned> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Ref::Borrowed(r) => r,
            Ref::Owned(owned) => owned.borrow(),
        }
    }
}
