use super::*;

use core::cmp;
use core::fmt;
use core::hash;
use core::mem;
use core::ops;

use crate::fatptr::FatPtr;
use crate::marshal::Persist;
use crate::coerce::{TryCast, TryCastRef, TryCastMut};

/// Wrapper around a `FatPtr` guaranteeing that the target of the pointer is valid.
///
/// Implements `Deref<Target=FatPtr>` so the fields of the wrapped pointer are available;
/// `DerefMut` is *not* implemented because mutating the wrapper pointer could invalidate it.
#[repr(transparent)]
pub struct ValidPtr<T: ?Sized + Pointee, P>(FatPtr<T,P>);

unsafe impl<T: ?Sized + Pointee, P> Persist for ValidPtr<T,P>
where P: Persist,
      T::Metadata: Persist,
{}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastRef<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&ValidPtr<T,Q>, Self::Error> {
        self.0.try_cast_ref()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastMut<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCastMut<Q>
{
    fn try_cast_mut(&mut self) -> Result<&mut ValidPtr<T,Q>, Self::Error> {
        self.0.try_cast_mut()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCast<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCast<Q>
{}

impl<T: ?Sized + Pointee, P> ops::Deref for ValidPtr<T,P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized + Pointee, P> ValidPtr<T,P> {
    /// Creates a new `ValidPtr` from a `FatPtr`.
    ///
    /// # Safety
    ///
    /// You are asserting that the pointer is in fact valid.
    pub unsafe fn new_unchecked(ptr: FatPtr<T,P>) -> Self {
        Self(ptr)
    }

    /// Unwraps the pointer.
    pub fn into_inner(self) -> FatPtr<T,P> {
        self.0
    }

    /// Gets mutable access to the raw pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe because changes to the raw pointer could make it invalid.
    pub unsafe fn raw_mut(&mut self) -> &mut P {
        &mut self.0.raw
    }
}

impl<T: ?Sized + Pointee, P> From<ValidPtr<T,P>> for FatPtr<T,P> {
    /// Forwards to `into_inner()`
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
