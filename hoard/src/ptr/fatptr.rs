use std::any;
use std::fmt;
use std::cmp;
use std::marker::PhantomData;

use thiserror::Error;

use super::*;

use crate::load::*;

#[repr(C)]
pub struct FatPtr<T: ?Sized, P, M = <T as Pointee>::Metadata> {
    marker: PhantomData<*const T>,
    pub raw: P,
    pub metadata: M,
}

impl<T: ?Sized + Pointee, P> FatPtr<T, P> {
    pub fn new(raw: P, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            raw, metadata,
        }
    }
}

impl<T: ?Sized, P, M> fmt::Debug for FatPtr<T, P, M>
where P: fmt::Debug,
      M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(any::type_name::<Self>())
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M: Clone> Clone for FatPtr<T, P, M> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            raw: self.raw.duplicate(),
            metadata: self.metadata.clone(),
        }
    }
}

impl<T: ?Sized + Pointee, P1, P2> PartialEq<FatPtr<T, P2>> for FatPtr<T, P1>
where P1: PartialEq<P2>
{
    fn eq(&self, other: &FatPtr<T, P2>) -> bool {
        self.raw == other.raw
            && self.metadata == other.metadata
    }
}

impl<T: ?Sized + Pointee, P> Eq for FatPtr<T, P>
where P: Eq
{}

#[derive(Debug, Error)]
#[error("fixme")]
pub enum ValidateBlobFatPtrError<P: fmt::Debug, M: fmt::Debug> {
    Ptr(P),
    Metadata(M),
}

impl<T: ?Sized, P, M> ValidateBlob for FatPtr<T, P, M>
where P: ValidateBlob,
      M: ValidateBlob,
{
    type Error = ValidateBlobFatPtrError<P::Error, M::Error>;

    const BLOB_LEN: usize = P::BLOB_LEN + M::BLOB_LEN;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_fields();
        blob.validate::<P>().map_err(ValidateBlobFatPtrError::Ptr)?;
        blob.validate::<M>().map_err(ValidateBlobFatPtrError::Metadata)?;
        unsafe { Ok(blob.assume_valid()) }
    }
}

unsafe impl <T: ?Sized, P, M> Persist for FatPtr<T, P, M>
where P: Persist,
      M: Persist,
{}

impl<Z, T: ?Sized, P, M> Load<Z> for FatPtr<T, P, M>
where P: Decode<Z>,
      M: Decode<Z>,
{
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Self::Owned {
        let mut loader = blob.into_loader(zone);

        unsafe {
            FatPtr {
                marker: PhantomData,
                raw: loader.decode_unchecked(),
                metadata: loader.decode_unchecked(),
            }
        }
    }
}
