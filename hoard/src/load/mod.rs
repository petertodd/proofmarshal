use std::task::Poll;

use crate::blob::{Blob, BlobDyn, Bytes};
use crate::pointee::Pointee;
use crate::owned::{Ref, IntoOwned};
use crate::zone::{PtrConst, PtrBlob, FromPtr};

pub mod impls;

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

pub trait LoadRef : Pointee + IntoOwned {
    type BlobDyn : BlobDyn<Metadata = Self::Metadata>;
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
}

/*
pub trait Validate : Load {
    type CleanPtr : PtrConst;
    type ValidateError : 'static + std::error::Error;
    type ValidatePoll : ValidatePoll<CleanPtr = Self::CleanPtr>;
}

pub trait ValidatePoll {
    type CleanPtr : PtrConst;
    type Error : 'static + std::error::Error;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = Self::CleanPtr>;

    fn validate_poll<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator,
              V::Ptr: FromPtr<Self::CleanPtr>;
}

pub trait Validator {
    type Ptr : PtrConst;
    type Error : 'static + std::error::Error;
}
*/
