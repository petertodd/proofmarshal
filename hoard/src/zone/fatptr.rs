//! Raw, *unvalidated*, zone pointers.

use std::alloc::Layout;
use std::any::type_name;
use std::cmp;
use std::fmt;
use std::hash;
use std::mem;

use thiserror::Error;
use nonzero::NonZero;

use crate::coerce::TryCoerce;
use crate::pointee::Pointee;

use crate::marshal::*;
use crate::marshal::blob::*;
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::load::PersistPointee;
use super::Zone;

/// A zone pointer with metadata. *Not* necessarily valid.
#[repr(C)]
pub struct FatPtr<T: ?Sized + Pointee, Z: Zone> {
    /// The pointer itself.
    pub raw: Z::Ptr,

    /// Metadata associated with this pointer.
    pub metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, Z: Zone> NonZero for FatPtr<T, Z> {}

unsafe impl<T1, T2, Z1, Z2> TryCoerce<FatPtr<T2, Z2>> for FatPtr<T1, Z1>
where T1: ?Sized + Pointee,
      T2: ?Sized + Pointee<Metadata=T1::Metadata>,
      Z1: Zone, Z2: Zone,
      Z1::Ptr: TryCoerce<Z2::Ptr>,
{
    type Error = <Z1::Ptr as TryCoerce<Z2::Ptr>>::Error;

    fn try_coerce_ptr(this: &Self) -> Result<*const FatPtr<T2, Z2>, Self::Error> {
        Z1::Ptr::try_coerce_ptr(&this.raw)?;

        assert_eq!(Layout::new::<Self>(), Layout::new::<FatPtr<T2, Z2>>());
        Ok(this as *const _ as *const _)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> FatPtr<T, Z> {
    pub fn cast<Y: Zone>(self) -> FatPtr<T, Y>
        where Z::Ptr: Into<Y::Ptr>
    {
        FatPtr {
            raw: self.raw.into(),
            metadata: self.metadata,
        }
    }
}

/// Returned when validation of a `FatPtr` blob fails.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ValidateFatPtrError<T: fmt::Debug, P: fmt::Debug> {
    #[error("invalid pointer: {0:?}")]
    Ptr(P),

    #[error("invalid metadata: {0:?}")]
    Metadata(T),
}

impl<T: ?Sized + Pointee, Z: Zone> ValidateBlob for FatPtr<T, Z>
where T::Metadata: ValidateBlob,
      Z::PersistPtr: ValidateBlob,
{
    type Error = ValidateFatPtrError<<T::Metadata as ValidateBlob>::Error,
                                     <Z::PersistPtr as ValidateBlob>::Error>;

    fn validate<V: PaddingValidator>(mut blob: BlobCursor<Self, V>)
        -> Result<ValidBlob<Self>, BlobError<Self::Error, V::Error>>
    {
        blob.field::<Z::PersistPtr, _>(ValidateFatPtrError::Ptr)?;
        blob.field::<T::Metadata, _>(ValidateFatPtrError::Metadata)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T: ?Sized + PersistPointee, Z: Zone> Persist for FatPtr<T, Z> {
    type Persist = FatPtr<T::Persist, Z::Persist>;
    type Error = <FatPtr<T::Persist, Z::Persist> as ValidateBlob>::Error;
}

unsafe impl<'a, T: ?Sized + PersistPointee, Z: Zone, Y> ValidateChildren<'a, Y> for FatPtr<T, Z> {
    type State = ();
    fn validate_children(this: &Self::Persist) -> () {}
    fn poll<V: PtrValidator<Y>>(_: &'a Self::Persist, _: &mut (), _: &V) -> Result<(), V::Error> {
        Ok(())
    }
}
impl<T: ?Sized + PersistPointee, Z: Zone, Y> Decode<Y> for FatPtr<T, Z> {}

impl<T: ?Sized + Pointee, Z: Zone, Y> Encoded<Y> for FatPtr<T,Z> {
    type Encoded = Self;
}

impl<'a, T: ?Sized + Pointee, Z: Zone, Y> Encode<'a, Y> for FatPtr<T,Z>
where Z::Ptr: Primitive
{
    type State = ();

    #[inline(always)]
    fn make_encode_state(&self) -> () {}

    #[inline(always)]
    fn encode_poll<D: Dumper<Y>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.raw)?
           .write_primitive(&self.metadata)?
           .finish()
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> Primitive for FatPtr<T,Z>
where Z::Ptr: Primitive
{}

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
