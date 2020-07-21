//! Loading of data behind zone pointers.

use std::error::Error;
use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops;

use thiserror::Error;

use owned::IntoOwned;

use crate::pointee::Pointee;
use crate::refs::Ref;
use crate::blob::*;
use crate::ptr::{Ptr, AsZone};

/// A type that can be loaded into memory.
pub trait Load : IntoOwned + ValidateBlob {
    /// The type of `Ptr` values of this type will contain.
    type Ptr : Ptr;

    fn decode_blob(blob: ValidBlob<Self>, zone: &<Self::Ptr as Ptr>::BlobZone) -> Self::Owned;

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &<Self::Ptr as Ptr>::BlobZone) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Err(blob)
    }
}

/// Automatically implemented for `Sized` types that implement `Load`.
pub trait Decode : Sized + Load + IntoOwned<Owned=Self> {
}

impl<T: Load + IntoOwned<Owned=Self>> Decode for T {
}

/// `Load`, but automatically implemented for any compatible pointer type.
pub trait LoadPtr<P: Ptr> : IntoOwned + ValidateBlob {
    fn decode_blob(blob: ValidBlob<Self>, zone: &P::BlobZone) -> Self::Owned;

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &P::BlobZone) -> Result<&'a Self, ValidBlob<'a, Self>>
    {
        Err(blob)
    }

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &P::BlobZone) -> Ref<'a, Self>
    {
        Ref::Owned(Self::decode_blob(blob, zone))
    }
}

/// `LoadPtr`, but for `Sized` types.
pub trait DecodePtr<P: Ptr> : Sized + LoadPtr<P, Owned=Self> {
}

impl<P: Ptr, T: Sized + LoadPtr<P, Owned=Self> + ValidateBlob> DecodePtr<P> for T {
}

impl<P: Ptr, T: ?Sized + IntoOwned + Load> LoadPtr<P> for T
where P::BlobZone: AsZone<<T::Ptr as Ptr>::BlobZone>
{
    fn decode_blob(blob: ValidBlob<Self>, zone: &P::BlobZone) -> Self::Owned {
        T::decode_blob(blob, zone.as_zone())
    }
}
