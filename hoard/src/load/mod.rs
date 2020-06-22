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
use crate::zone::*;

pub trait Load : IntoOwned + ValidateBlobDyn<padding::CheckPadding> + ValidateBlobDyn<padding::IgnorePadding> {
    /// The type of `Zone` present within a value of this type.
    type Zone : Zone;

    /// The type of `Ptr` present within a value of this type.
    type Ptr : Ptr;

    fn load_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self::Owned;

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Ref<'a, Self> {
        match Self::try_deref_blob(blob, zone) {
            Ok(r) => Ref::Ref(r),
            Err(blob) => Ref::Owned(Self::load_blob(blob, zone)),
        }
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Result<&'a Self, ValidBlob<'a, Self>> {
        let _ = zone;
        Err(blob)
    }
}

pub trait Decode : Sized + ValidateBlob<padding::CheckPadding> + ValidateBlob<padding::IgnorePadding> {
    type Zone : Zone;

    /// The type of `Ptr` present within a value of this type.
    type Ptr : Ptr;

    fn decode_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self;

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Ref<'a, Self> {
        match Self::try_deref_blob(blob, zone) {
            Ok(r) => Ref::Ref(r),
            Err(blob) => Ref::Owned(Self::decode_blob(blob, zone)),
        }
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Result<&'a Self, ValidBlob<'a, Self>> {
        let _ = zone;
        Err(blob)
    }
}

impl<T: Decode> Load for T {
    type Zone = T::Zone;

    /// The type of `Ptr` present within a value of this type.
    type Ptr = T::Ptr;

    fn load_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self {
        Self::decode_blob(blob, zone)
    }

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Ref<'a, Self> {
        Self::deref_blob(blob, zone)
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Self::Zone) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Self::try_deref_blob(blob, zone)
    }
}

/*
pub trait DecodeIn<Z: Zone> : Sized + ValidateBlob<padding::CheckPadding> + ValidateBlob<padding::IgnorePadding> {
    fn decode_blob_in(blob: ValidBlob<Self>, zone: &Z) -> Self;

    fn deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self> {
        match Self::try_deref_blob_in(blob, zone) {
            Ok(r) => Ref::Ref(r),
            Err(blob) => Ref::Owned(Self::decode_blob_in(blob, zone)),
        }
    }

    fn try_deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Result<&'a Self, ValidBlob<'a, Self>> {
        let _ = zone;
        Err(blob)
    }
}

impl<Z: Zone, T: Decode> DecodeIn<Z> for T
where Z: AsZone<T::Zone>
{
    fn decode_blob_in(blob: ValidBlob<Self>, zone: &Z) -> Self {
        T::decode_blob(blob, zone.as_zone())
    }

    fn deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self> {
        T::deref_blob(blob, zone.as_zone())
    }

    fn try_deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Result<&'a Self, ValidBlob<'a, Self>> {
        T::try_deref_blob(blob, zone.as_zone())
    }
}

pub trait LoadIn<Z> : Load {
    fn load_blob_in(blob: ValidBlob<Self>, zone: &Z) -> Self::Owned;
    fn decode_blob_in(blob: ValidBlob<Self>, zone: &Z) -> Self
        where Self: Sized
    {
        let owned = Self::load_blob_in(blob, zone);
        todo!()
    }
    fn deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self>;
}

impl<Z: Zone, T: ?Sized + Load> LoadIn<Z> for T
where Z: AsZone<T::Zone>,
{
    fn load_blob_in(blob: ValidBlob<Self>, zone: &Z) -> Self::Owned {
        T::load_blob(blob, zone.as_zone())
    }

    fn deref_blob_in<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self> {
        T::deref_blob(blob, zone.as_zone())
    }
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
