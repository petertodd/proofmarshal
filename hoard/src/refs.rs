use super::*;

use std::borrow::Borrow;
use std::cmp;
use std::fmt;
use std::ops;

/// A reference to a value in a zone.
pub enum Ref<'p, T: ?Sized + Load<Z>, Z: Zone> {
    Borrowed(&'p T),
    Owned(T::Owned),
}

impl<'p, T: ?Sized + Load<Z>, Z: Zone> Ref<'p, T, Z> {
    pub fn map<F, U: ?Sized + Load<Z>>(self, f: F) -> Ref<'p, U, Z>
        where F: for<'q> FnOnce(&'q T) -> &'q U,
              U: ToOwned<Owned=<U as Load<Z>>::Owned>,
    {
        match self.try_map::<_,U,!>(|b| Ok(f(b))) {
            Ok(r) => r,
            Err(never) => never,
        }
    }

    pub fn try_map<F, U: ?Sized + Load<Z>, E>(self, f: F) -> Result<Ref<'p, U, Z>, E>
        where F: for<'q> FnOnce(&'q T) -> Result<&'q U, E>,
              U: ToOwned<Owned = <U as Load<Z>>::Owned>,
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

impl<T: ?Sized + Load<Z>, Z: Zone> ops::Deref for Ref<'_,T,Z> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(o) => o.borrow(),
        }
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> fmt::Debug for Ref<'_,T,Z>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
