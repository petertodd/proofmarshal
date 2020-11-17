//! Loading data behind zone pointers.

use std::task::Poll;

use crate::blob::{Blob, BlobDyn, Bytes};
use crate::pointee::Pointee;
use crate::owned::{Ref, IntoOwned};
use crate::ptr::{Ptr, PtrClean, AsZone};

pub use crate::validate::MaybeValid;

pub mod impls;

/// A sized type with a `Blob` serializaton.
pub trait Load : Sized {
    /// The `Blob` form of this type.
    type Blob : Blob;

    /// Used to access *clean* data referenced by values of this type.
    type PtrClean : PtrClean<Zone = Self::Zone>;

    /// The zone needed by pointers within a value of this type.
    type Zone;

    /// Loads a blob using the provided zone, returning a value with the appropriate zones added to
    /// all pointers.
    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self;

    /// Loads a blob that may or may not be valid.
    fn load_maybe_valid(blob: MaybeValid<Self::Blob>, zone: &Self::Zone) -> MaybeValid<Self> {
        Self::load(blob.trust(), zone).into()
    }

    /// Loads a `Ref` directly from bytes.
    fn load_bytes<'a>(bytes: Bytes<'a, Self::Blob>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::Blob as Blob>::DecodeBytesError>
    {
        let blob = <Self::Blob as Blob>::decode_bytes(bytes)?
                                        .trust();
        let this = Self::load(blob, zone);

        Ok(MaybeValid::from(Ref::<Self>::Owned(this)))
    }
}

/// Loading of potentially unsized data behind pointers.
///
/// There is a blanket implementation of `LoadRef` for all `T: Load`.
pub trait LoadRef : Pointee + IntoOwned {
    /// The dynamically sized, blob form of this type.
    type BlobDyn : ?Sized + BlobDyn + Pointee<Metadata = <Self as Pointee>::Metadata>;

    /// The type of *clean* pointer that is present in values of this type.
    type PtrClean : PtrClean<Zone = Self::Zone>;

    /// The zone needed by pointers within a value of this type.
    type Zone;

    /// Loads the owned form of this type directly from bytes.
    ///
    /// For example, the owned form of `[T]` slice is a `Vec<T>`. So this function would allow you
    /// to deserialize raw bytes into a `Vec<T>`.
    fn load_owned_from_bytes(bytes: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>;

    /// Loads a `Ref` directly from bytes.
    fn load_ref_from_bytes<'a>(bytes: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        Self::load_owned_from_bytes(bytes, zone)
             .map(|owned| MaybeValid::from(Ref::Owned(owned.trust())))
    }
}

impl<T: Load> LoadRef for T {
    type BlobDyn = T::Blob;
    type PtrClean = T::PtrClean;
    type Zone = T::Zone;

    fn load_ref_from_bytes<'a>(bytes: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        T::load_bytes(bytes, zone)
    }

    fn load_owned_from_bytes(bytes: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <T::Blob as Blob>::decode_bytes(bytes)?
                                     .trust();
        let this = T::load(blob, zone);
        Ok(MaybeValid::from(this))
    }
}
