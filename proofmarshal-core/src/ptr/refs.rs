use super::*;

use core::ops;
use core::cmp;

#[derive(Debug)]
pub enum Ref<'a, T: ?Sized + Type<P>, P> {
    Borrowed(&'a T::Type),
    Owned(T::Owned),
}

impl<T: ?Sized + Type<P>, P> Ref<'_,T,P> {
    /// Extracts the owned data.
    ///
    /// Clones the data if it is not already owned.
    pub fn into_owned(self) -> T::Owned {
        match self {
            Ref::Borrowed(r) => r.to_owned(),
            Ref::Owned(owned) => owned,
        }
    }
}

impl<T: ?Sized + Type<P>, P> ops::Deref for Ref<'_, T, P> {
    type Target = T::Type;

    fn deref(&self) -> &T::Type {
        match self {
            Ref::Borrowed(r) => r,
            Ref::Owned(owned) => owned.borrow(),
        }
    }
}

impl<T: ?Sized + Type<P> + Type<Q>, P, Q> cmp::PartialEq<Ref<'_,T,Q>> for Ref<'_,T,P>
where <T as Coerce<P>>::Type: cmp::PartialEq<<T as Coerce<Q>>::Type>,
{
    fn eq(&self, other: &Ref<'_,T,Q>) -> bool {
        (**self).eq(&**other)
    }
    fn ne(&self, other: &Ref<'_,T,Q>) -> bool {
        (**self).ne(&**other)
    }
}

impl<T: ?Sized + Type<P>, P> cmp::Eq for Ref<'_,T,P>
where T::Type: cmp::Eq
{}
