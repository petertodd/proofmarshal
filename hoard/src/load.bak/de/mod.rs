//! Deserialization of pointer-containing types.

use core::any::Any;
use core::convert::TryFrom;
use core::fmt;
use core::mem::{self, MaybeUninit};
use core::slice;

use owned::Owned;
use crate::zone::*;
use crate::pointee::*;

use super::Freeze;
use super::blob::*;
use super::en::{Save, Encode};
use super::primitive::Primitive;

/// A type whose values can be loaded from pointers in a zone.
pub unsafe trait Load<P: Ptr> : Save<P> + Freeze {
    /// Error returned when `Blob` validation fails.
    type Error : 'static + fmt::Debug;

    /// What validates children of this type.
    type ChildValidator : ChildValidator<P>;

    /// Validates a blob.
    ///
    /// Immediately returns an error if the blob itself is structurally invalid. Otherwise returns
    /// a `BlobValidator` which can be further validated.
    ///
    /// # Safety
    ///
    /// The implementer guarantees that once `ChildValidator::poll()` returns `Ok`, the returned
    /// blob is fully valid and can be dereferenced as a `&'a Self` without any further
    /// restrictions.
    ///
    /// This guarantee is why this trait is marked `unsafe`.
    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<BlobValidator<'a, Self, P>, Self::Error>;
}

/// Validation of child pointers.
pub trait ChildValidator<P: Ptr> {
    fn poll<V>(&mut self, ptr_validator: &V) -> Result<(), V::Error>
        where V: PtrValidator<P>;
}

/// `ChildValidator` for types that don't contain any internal pointers.
impl<P: Ptr> ChildValidator<P> for () {
    /// Unconditionally returns `Ok(())`.
    fn poll<V>(&mut self, _: &V) -> Result<(), V::Error>
        where V: PtrValidator<P>
    {
        Ok(())
    }
}

/// `Load` but for sized types.
pub unsafe trait Decode<P: Ptr> : Encode<P> + Freeze {
    /// Error returned when `Blob` validation fails.
    type Error : 'static + fmt::Debug;

    /// What validates children of this type.
    type ChildValidator : ChildValidator<P>;

    /// Validates a blob.
    ///
    /// Returns either a `ValidBlob`, which can be further validated, or an error.
    ///
    /// # Safety
    ///
    /// The implementer guarantees that once `ChildValidator::poll()` returns `Ok`, the returned
    /// blob is fully valid and can be dereferenced as a `&'a Self` without any further
    /// restrictions.
    ///
    /// This guarantee is why this trait is marked `unsafe`.
    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<BlobValidator<'a, Self, P>, Self::Error>;
}

unsafe impl<P: Ptr, T: Decode<P>> Load<P> for T {
    type Error = T::Error;

    type ChildValidator = T::ChildValidator;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<BlobValidator<'a, Self, P>, Self::Error> {
        T::validate_blob(blob)
    }
}

unsafe impl<P: Ptr, T: Primitive> Decode<P> for T {
    type Error = T::Error;

    type ChildValidator = ();
    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<BlobValidator<'a, Self, P>, Self::Error> {
        T::validate_blob(blob)
            .map(|valid_blob| BlobValidator::from(valid_blob))
    }
}

pub unsafe trait PtrValidator<P: Ptr> {
    type Error;

    fn validate_ptr<T: ?Sized + Load<P>>(&self, ptr: &FatPtr<T, P::Persist>)
        -> Result<Option<T::ChildValidator>,
                  Self::Error>;
}
