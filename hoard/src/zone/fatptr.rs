//! Raw, *unvalidated*, zone pointers.

use core::cmp;
use core::fmt;
use core::hash;

use super::*;

use crate::{
    coerce::{CastRef, TryCastRef, TryCast, TryCastMut},
    marshal::{
        blob::{Blob, ValidBlob, WriteBlob},
        primitive::Primitive,
    },
};

/// A zone pointer with metadata. *Not* necessarily valid.
#[repr(C)]
pub struct FatPtr<T: ?Sized + Pointee, P> {
    /// The pointer itself.
    pub raw: P,

    /// Metadata associated with this pointer.
    pub metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, P> NonZero for FatPtr<T,P>
where P: NonZero {}

/// Returned when validation of a `FatPtr` blob fails.
#[derive(Debug, PartialEq, Eq)]
pub enum ValidateError<T, P> {
    Ptr(P),
    Metadata(T),
}

unsafe impl<T: ?Sized + Pointee, P> Primitive for FatPtr<T,P>
where P: Primitive
{
    type Error = ValidateError<<T::Metadata as Primitive>::Error, P::Error>;
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.raw)?
           .write_primitive(&self.metadata)?
           .finish()
    }

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_primitive_struct();
        blob.field::<P>().map_err(ValidateError::Ptr)?;
        blob.field::<T::Metadata>().map_err(ValidateError::Metadata)?;

        Ok(unsafe { blob.done() })
    }
}

/// Sized types have no pointer metadata, allowing pointers to them to be converted directly to fat
/// pointers.
impl<T, P> From<P> for FatPtr<T,P> {
    fn from(raw: P) -> Self {
        Self { raw, metadata: () }
    }
}

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

// standard impls

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for FatPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FatPtr")
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<FatPtr<T,Q>> for FatPtr<T,P>
where P: cmp::PartialEq<Q>,
{
    fn eq(&self, other: &FatPtr<T,Q>) -> bool {
        (self.raw == other.raw) && (self.metadata == other.metadata)
    }
}

impl<T: ?Sized + Pointee, P> cmp::Eq for FatPtr<T,P>
where P: cmp::Eq,
{}

impl<T: ?Sized + Pointee, P> Clone for FatPtr<T,P>
where P: Clone,
{
    fn clone(&self) -> Self {
        Self { raw: self.raw.clone(), metadata: self.metadata }
    }
}

impl<T: ?Sized + Pointee, P> Copy for FatPtr<T,P>
where P: Copy,
{}

impl<T: ?Sized + Pointee, P> hash::Hash for FatPtr<T,P>
where P: hash::Hash,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.metadata.hash(state);
    }
}


impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for FatPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FatPtr")
            .field(&self.raw)
            .field(&self.metadata)
            .finish()
    }
}
