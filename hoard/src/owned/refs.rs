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

impl<A: ?Sized + IntoOwned, B: ?Sized + IntoOwned> PartialEq<Ref<'_, B>> for Ref<'_, A>
where A: PartialEq<B>
{
    fn eq(&self, other: &Ref<'_, B>) -> bool {
        self.deref() == other.deref()
    }

    fn ne(&self, other: &Ref<'_, B>) -> bool {
        self.deref() != other.deref()
    }
}

impl<A: ?Sized + IntoOwned, B: ?Sized + IntoOwned> PartialEq<&'_ B> for Ref<'_, A>
where A: PartialEq<B>
{
    fn eq(&self, other: &&B) -> bool {
        self.deref() == *other
    }

    fn ne(&self, other: &&B) -> bool {
        self.deref() != *other
    }
}
