use super::*;

use core::cmp;
use core::fmt;
use core::hash;
use core::ops;

use crate::fatptr::FatPtr;
use crate::marshal::Persist;

#[repr(transparent)]
pub struct ValidPtr<T: ?Sized + Pointee, P>(FatPtr<T,P>);

unsafe impl<T: ?Sized + Pointee, P> Persist for ValidPtr<T,P>
where P: Persist,
      T::Metadata: Persist,
{}

impl<T: ?Sized + Pointee, P> ops::Deref for ValidPtr<T,P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized + Pointee, P> ValidPtr<T,P> {
    pub unsafe fn new_unchecked(ptr: FatPtr<T,P>) -> Self {
        Self(ptr)
    }

    pub fn into_inner(self) -> FatPtr<T,P> {
        self.0
    }
}

impl<T: ?Sized + Pointee, P> From<ValidPtr<T,P>> for FatPtr<T,P> {
    fn from(valid: ValidPtr<T,P>) -> Self {
        valid.into_inner()
    }
}

// standard impls
impl<T: ?Sized + Pointee, P> fmt::Debug for ValidPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ValidPtr")
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for ValidPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &ValidPtr<T,Q>) -> bool {
        &self.0 == &other.0
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<FatPtr<T,Q>> for ValidPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &FatPtr<T,Q>) -> bool {
        &self.0 == other
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<ValidPtr<T,Q>> for FatPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &ValidPtr<T,Q>) -> bool {
        self == &other.0
    }
}

impl<T: ?Sized + Pointee, P> cmp::Eq for ValidPtr<T,P>
where P: cmp::Eq {}

impl<T: ?Sized + Pointee, P> Clone for ValidPtr<T,P>
where P: Clone
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: ?Sized + Pointee, P> Copy for ValidPtr<T,P>
where P: Copy {}

impl<T: ?Sized + Pointee, P> hash::Hash for ValidPtr<T,P>
where P: hash::Hash,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}
