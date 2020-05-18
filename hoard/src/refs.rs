use std::borrow::Borrow;
use std::ops::Deref;
use std::fmt;

use owned::IntoOwned;

pub enum Ref<'a, T: ?Sized + IntoOwned> {
    Ref(&'a T),
    Owned(T::Owned),
}

impl<'a, T: ?Sized + IntoOwned> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Ref::Ref(r) => r,
            Ref::Owned(owned) => owned.borrow(),
        }
    }
}

impl<'a, T: ?Sized + IntoOwned> fmt::Debug for Ref<'a, T>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.deref().fmt(f)
    }
}

impl<'a, T: ?Sized + IntoOwned> From<&'a T> for Ref<'a, T> {
    fn from(r: &'a T) -> Self {
        Ref::Ref(r)
    }
}
