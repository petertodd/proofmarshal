use std::task::Poll;
use std::marker::PhantomData;

pub mod bytes;
pub use self::bytes::{Bytes, BytesUninit, ValidBytes};

pub mod impls;

use crate::pointee::Pointee;
use crate::owned::IntoOwned;
use crate::zone::{AsPtr, FromPtr, IntoPtr, PtrBlob};

pub use crate::maybevalid::MaybeValid;

pub trait Blob : 'static + Sized {
    const SIZE: usize;
    type DecodeBytesError : 'static + std::error::Error;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError>;
}

/// Dynamically sized blob.
pub unsafe trait BlobDyn : 'static + Pointee + IntoOwned {
    type DecodeBytesError : 'static + std::error::Error;

    fn try_size(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError>;
}

unsafe impl<T: Blob> BlobDyn for T {
    type DecodeBytesError = T::DecodeBytesError;

    fn try_size(_: ()) -> Result<usize, !> {
        Ok(Self::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        self.encode_bytes(dst)
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        T::decode_bytes(blob)
    }
}

/*
pub trait ValidateImpl {
    type Ptr : PtrBlob;
    type Error : 'static + std::error::Error;
}

pub trait ValidateBlob<'a> : ValidateImpl {
    type ValidatePoll : ValidatePoll<Ptr = Self::Ptr, Error = Self::Error>;

    fn init_validate_blob(&'a self) -> Self::ValidatePoll;
}

pub trait ValidateBlobDyn<'a> : ValidateImpl {
    type ValidatePoll : ValidatePoll<Ptr = Self::Ptr, Error = Self::Error>;

    fn init_validate_blob_dyn(&'a self) -> Self::ValidatePoll;
}

pub trait ValidatePoll {
    type Ptr : PtrBlob;
    type Error : 'static + std::error::Error;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = Self::Ptr>;

    fn validate_poll<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator,
              Self::Ptr: AsPtr<V::Ptr>,
    {
        todo!()
    }
}

impl ValidatePoll for () {
    type Ptr = !;
    type Error = !;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }
}

pub trait Validator {
    type Ptr : PtrBlob;
    type Error : 'static + std::error::Error;
}
*/
