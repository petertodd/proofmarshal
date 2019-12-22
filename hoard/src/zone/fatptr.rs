//! Raw, *unvalidated*, zone pointers.

use core::cmp;
use core::fmt;
use core::hash;

use nonzero::NonZero;

use super::*;

use crate::load::{Validate, ValidationError};
use crate::blob::{BlobValidator, StructValidator};

/// A zone pointer with metadata. *Not* necessarily valid.
#[repr(C)]
pub struct FatPtr<T: ?Sized + Pointee, Z: Zone> {
    /// The pointer itself.
    pub raw: Z::Ptr,

    /// Metadata associated with this pointer.
    pub metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, Z: Zone> NonZero for FatPtr<T, Z> {}

/// Returned when validation of a `FatPtr` blob fails.
#[derive(Debug, PartialEq, Eq)]
pub enum ValidateFatPtrError<T, P> {
    Ptr(P),
    Metadata(T),
}

impl<T: ValidationError, P: ValidationError> ValidationError for ValidateFatPtrError<T, P> {
}

impl<T: ?Sized + Validate, Z: PersistZone> Validate for FatPtr<T, Z> {
    type Error = ValidateFatPtrError<<T::Metadata as Validate>::Error,
                                     <Z::PersistPtr as Validate>::Error>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();

        blob.field::<Z::PersistPtr, _>(ValidateFatPtrError::Ptr)?;
        blob.field::<T::Metadata, _>(ValidateFatPtrError::Metadata)?;

        unsafe { blob.assume_valid() }
    }
}

/*
unsafe impl<T: ?Sized + Pointee, P, Q> TryCastRef<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&FatPtr<T,Q>, Self::Error> {
        match self.raw.try_cast_ref() {
            Err(e) => Err(e),
            Ok(_) => Ok(unsafe { &*(self as *const _ as *const _) })
        }
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastMut<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCastMut<Q>
{
    fn try_cast_mut(&mut self) -> Result<&mut FatPtr<T,Q>, Self::Error> {
        match self.raw.try_cast_mut() {
            Err(e) => Err(e),
            Ok(_) => Ok(unsafe { &mut *(self as *mut _ as *mut _) })
        }
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCast<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCast<Q>
{}

impl<T: ?Sized + Pointee, P, Q> AsRef<FatPtr<T,Q>> for FatPtr<T,P>
where P: CastRef<Q>
{
    fn as_ref(&self) -> &FatPtr<T,Q> {
        unsafe {
            &*(self as *const _ as *const _)
        }
    }
}
*/

// standard impls

impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for FatPtr<T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FatPtr")
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Pointer for FatPtr<T, Z>
where Z::Ptr: fmt::Pointer
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.raw, f)
    }
}

impl<T: ?Sized + Pointee, Z: Zone, Y: Zone> cmp::PartialEq<FatPtr<T,Y>> for FatPtr<T,Z>
where Z::Ptr: cmp::PartialEq<Y::Ptr>,
{
    fn eq(&self, other: &FatPtr<T, Y>) -> bool {
        (self.raw == other.raw) && (self.metadata == other.metadata)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> cmp::Eq for FatPtr<T, Z>
{}

impl<T: ?Sized + Pointee, Z: Zone> Clone for FatPtr<T, Z> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized + Pointee, Z: Zone> Copy for FatPtr<T, Z> {}

impl<T: ?Sized + Pointee, Z: Zone> hash::Hash for FatPtr<T, Z> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.metadata.hash(state);
    }
}

// TODO: PartialOrd/Ord
