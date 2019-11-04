//! Targets of pointers.

use std::fmt;
use std::borrow::Borrow;
use std::ops;
use std::cmp;
use std::hash;

/// The owned form of a type.
pub trait Owned {
    type Owned : Borrow<Self>;
}

impl<T> Owned for T {
    type Owned = T;
}

impl<T> Owned for [T] {
    type Owned = Vec<T>;
}

/// A reference that may be a true reference, or an owned value.
pub enum Ref<'a, B: ?Sized + Owned> {
    Borrowed(&'a B),
    Owned(<B as Owned>::Owned),
}

impl<B: ?Sized + Owned> fmt::Debug for Ref<'_, B>
where B: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<B: ?Sized + Owned> fmt::Display for Ref<'_, B>
where B: fmt::Display
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<B: Clone> Clone for Ref<'_, B> {
    fn clone(&self) -> Self {
        match self {
            Ref::Borrowed(r) => Ref::Borrowed(r),
            Ref::Owned(owned) => Ref::Owned(owned.clone()),
        }
    }
}

impl<B: ?Sized + Owned> ops::Deref for Ref<'_, B> {
    type Target = B;

    fn deref(&self) -> &B {
        match self {
            Ref::Borrowed(r) => r,
            Ref::Owned(owned) => owned.borrow(),
        }
    }
}

impl<'b,'c, B: ?Sized + Owned, C: ?Sized + Owned> cmp::PartialEq<Ref<'c, C>> for Ref<'b, B>
where B: cmp::PartialEq<C>,
{
    fn eq(&self, other: &Ref<'c, C>) -> bool {
        cmp::PartialEq::eq(&**self, &**other)
    }
}
impl<B: ?Sized + Owned> cmp::Eq for Ref<'_, B>
where B: cmp::Eq,
{}

impl<'b,'c, B: ?Sized + Owned, C: ?Sized + Owned> cmp::PartialOrd<Ref<'c, C>> for Ref<'b, B>
where B: cmp::PartialOrd<C>,
{
    fn partial_cmp(&self, other: &Ref<'c, C>) -> Option<cmp::Ordering> {
        cmp::PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<B: ?Sized + Owned> cmp::Ord for Ref<'_, B>
where B: cmp::Ord,
{
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        cmp::Ord::cmp(&**self, &**other)
    }
}

impl<B: ?Sized + Owned> hash::Hash for Ref<'_, B>
where B: hash::Hash,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        hash::Hash::hash(&**self, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_deref() {
        let v = vec![1u8,2,3];
        let r = Ref::Borrowed(&v);

        assert_eq!(r.len(), 3);

        let r: Ref<Vec<u8>> = Ref::Owned(v);
        assert_eq!(r.len(), 3);
    }
}
