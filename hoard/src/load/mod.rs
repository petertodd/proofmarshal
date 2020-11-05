use std::task::Poll;

use crate::blob::{Blob, BlobDyn, Bytes};
use crate::pointee::Pointee;
use crate::owned::{Ref, IntoOwned};
use crate::ptr::AsZone;

pub use crate::maybevalid::MaybeValid;

pub trait Load : Sized {
    type Blob : Blob;
    type Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self;

    fn load_maybe_valid(blob: MaybeValid<Self::Blob>, zone: &Self::Zone) -> MaybeValid<Self> {
        Self::load(blob.trust(), zone).into()
    }

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

pub trait LoadIn<Z> : Sized {
    type Blob : Blob;

    fn load_in(blob: Self::Blob, zone: &Z) -> Self;
    fn load_maybe_valid_in(blob: MaybeValid<Self::Blob>, zone: &Z) -> MaybeValid<Self>;

    fn load_bytes_in<'a>(bytes: Bytes<'a, Self::Blob>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::Blob as Blob>::DecodeBytesError>;
}

impl<T: Load, Z> LoadIn<Z> for T
where Z: AsZone<T::Zone>
{
    type Blob = T::Blob;

    fn load_in(blob: Self::Blob, zone: &Z) -> Self {
        T::load(blob, zone.as_zone())
    }

    fn load_maybe_valid_in(blob: MaybeValid<Self::Blob>, zone: &Z) -> MaybeValid<Self> {
        T::load_maybe_valid(blob, zone.as_zone())
    }

    fn load_bytes_in<'a>(bytes: Bytes<'a, Self::Blob>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::Blob as Blob>::DecodeBytesError>
    {
        T::load_bytes(bytes, zone.as_zone())
    }
}

pub trait LoadRef : Pointee + IntoOwned {
    type BlobDyn : ?Sized + BlobDyn + Pointee<Metadata = <Self as Pointee>::Metadata>;
    type Zone;

    fn load_ref_from_bytes<'a>(bytes: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>;

    fn load_owned_from_bytes(bytes: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        todo!()
    }
}

impl<T: Load> LoadRef for T {
    type BlobDyn = T::Blob;
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

pub trait LoadRefIn<Z> : Pointee + IntoOwned {
    type BlobDyn : ?Sized + BlobDyn + Pointee<Metadata = <Self as Pointee>::Metadata>;

    fn load_ref_from_bytes_in<'a>(bytes: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>;

    fn load_owned_from_bytes_in(bytes: Bytes<'_, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Self::Owned>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>;
}

impl<T: ?Sized + LoadRef, Z> LoadRefIn<Z> for T
where Z: AsZone<T::Zone>
{
    type BlobDyn = T::BlobDyn;

    fn load_ref_from_bytes_in<'a>(bytes: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        Self::load_ref_from_bytes(bytes, zone.as_zone())
    }

    fn load_owned_from_bytes_in(bytes: Bytes<'_, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Self::Owned>,
                  <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        Self::load_owned_from_bytes(bytes, zone.as_zone())
    }
}
