use super::*;

use std::borrow::Borrow;
use std::cmp;
use std::fmt;
use std::ops;

/// A reference to a value in a zone.
pub enum Ref<'p, T: ?Sized + Save<Z>, Z: Zone> {
    Borrowed(&'p T),
    Owned(T::Owned),
}

/*
impl<'p, T: ?Sized + Store<Z>, Z> Ref<'p, T, Z> {
    pub fn map<F, U: ?Sized + Store<Z>>(self, f: F) -> Ref<'p, U, Z>
        where F: for<'q> FnOnce(&'q T) -> &'q U,
              U: ToOwned<Owned=<U as Store<Z>>::Owned>,
    {
        match self.try_map::<_,U,!>(|b| Ok(f(b))) {
            Ok(r) => r,
            Err(never) => never,
        }
    }

    pub fn try_map<F, U: ?Sized + Store<Z>, E>(self, f: F) -> Result<Ref<'p, U, Z>, E>
        where F: for<'q> FnOnce(&'q T) -> Result<&'q U, E>,
              U: ToOwned<Owned = <U as Store<Z>>::Owned>,
    {
        match self {
            Ref::Borrowed(borrowed) => Ok(Ref::Borrowed(f(borrowed)?)),
            Ref::Owned(owned) => {
                let mapped = f(owned.borrow())?;
                Ok(Ref::Owned(mapped.to_owned()))
            },
        }
    }
}

/*
impl<T: ?Sized + ToOwned, Z> Clone for Ref<'_, T, Z> {
    fn clone(&self) -> Self {
        match self {
            Ref::Borrowed(borrowed) => Ref::borrowed(borrowed),
            Ref::Owned(owned) => Ref::owned(owned.borrow().to_owned()),
        }
    }
}
*/
*/

impl<Z: Zone, T: ?Sized + Save<Z>> ops::Deref for Ref<'_,T,Z> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(o) => o.borrow(),
        }
    }
}

impl<Z: Zone, T: ?Sized + Save<Z>> fmt::Debug for Ref<'_,T,Z>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/*
impl<T1: ?Sized + Store<Z1>, Z1, T2: ?Sized + Store<Z2>, Z2> cmp::PartialEq<Ref<'_, T2, Z2>> for Ref<'_,T1,Z1>
where T1: cmp::PartialEq<T2>
{
    fn eq(&self, other: &Ref<T2,Z2>) -> bool {
        cmp::PartialEq::eq(&**self, &**other)
    }
}

impl<T: ?Sized + Store<Z>, Z> cmp::Eq for Ref<'_,T,Z>
where T: cmp::Eq
{}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
*/
