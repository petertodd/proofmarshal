//! Raw, *unvalidated*, zone pointers.

use core::cmp;
use core::fmt;
use core::hash;
use core::mem;

use nonzero::NonZero;

use super::*;

use crate::load::*;
use crate::save::*;
use crate::blob::*;

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

impl<T: ?Sized + Persist, Z: Zone> Persist for FatPtr<T, Z> {
    type Persist = FatPtr<T::Persist, Z::Persist>;
    type Error = ValidateFatPtrError<<T::Metadata as Persist>::Error,
                                     <Z::PersistPtr as Persist>::Error>;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<Z::PersistPtr, _>(ValidateFatPtrError::Ptr)?;
        blob.field::<T::Metadata, _>(ValidateFatPtrError::Metadata)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<'a, T: ?Sized + Persist, Z: Zone> Validate<'a, Z> for FatPtr<T,Z> {
    type State = ();
    fn validate_children(this: &Self::Persist) -> () {}
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, _: &mut (), _: &V) -> Result<&'a Self, V::Error> {
        Ok(unsafe { mem::transmute(this) })
    }
}
impl<T: ?Sized + Persist, Z: Zone> Decode<Z> for FatPtr<T,Z> {}

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
