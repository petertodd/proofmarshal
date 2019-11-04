use super::*;

use core::cmp;
use core::fmt;
use core::ops;

/// A reference to a value in a zone.
pub enum Ref<'p, T: ?Sized + Load<Z>, Z: Zone> {
    Borrowed(&'p T),
    Owned(<T as Load<Z>>::Owned),
}

impl<T: ?Sized + Load<Z>, Z: Zone> ops::Deref for Ref<'_,T,Z> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Ref::Borrowed(r) => r,
            Ref::Owned(owned) => owned.borrow(),
        }
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> fmt::Debug for Ref<'_,T,Z>
where T: fmt::Debug,
      <T as Load<Z>>::Owned: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ref::Borrowed(r) => fmt::Debug::fmt(r, f),
            Ref::Owned(o) => fmt::Debug::fmt(o, f),
        }
    }
}

#[allow(unreachable_code)]
#[derive(Debug)]
pub struct NeverOwned<T: ?Sized> {
    marker: PhantomData<T>,
    never: !,
}

impl<T: ?Sized> Borrow<T> for NeverOwned<T> {
    fn borrow(&self) -> &T {
        match self.never {}
    }
}

impl<T1: ?Sized + Load<Z1>, Z1: Zone, T2: ?Sized + Load<Z2>, Z2: Zone> cmp::PartialEq<Ref<'_, T2, Z2>> for Ref<'_,T1,Z1>
where T1: cmp::PartialEq<T2>
{
    fn eq(&self, other: &Ref<T2,Z2>) -> bool {
        cmp::PartialEq::eq(&**self, &**other)
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> cmp::Eq for Ref<'_,T,Z>
where T: cmp::Eq
{}
