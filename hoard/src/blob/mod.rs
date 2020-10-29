use std::convert::TryFrom;
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
    type DecodeBytesError : 'static + std::error::Error + Send;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError>;

    /// Convenience wrapper around `encode_bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hoard::blob::Blob;
    ///
    /// assert_eq!(0x1234_u16.to_blob_bytes(),
    ///            vec![0x34, 0x12]);
    /// ```
    fn to_blob_bytes(&self) -> Vec<u8> {
        let mut r = vec![0u8; Self::SIZE];
        let dst = BytesUninit::try_from(&mut *r).unwrap();
        let _ = self.encode_bytes(dst);
        r
    }
}

/// Dynamically sized blob.
pub unsafe trait BlobDyn : 'static + Pointee + IntoOwned {
    type DecodeBytesError : 'static + std::error::Error;

    fn try_size(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError>;

    /// Convenience wrapper around `encode_bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hoard::blob::BlobDyn;
    ///
    /// // FIXME: use an actual unsized type
    /// assert_eq!(0x1234_u16.to_blob_bytes_dyn(),
    ///            vec![0x34, 0x12]);
    /// ```
    fn to_blob_bytes_dyn(&self) -> Vec<u8> {
        let metadata = Self::metadata(self);
        let size = Self::try_size(metadata).expect("valid metadata");
        let mut r = vec![0u8; size];
        let dst = BytesUninit::from_bytes(&mut *r, metadata).unwrap();
        let _ = self.encode_bytes(dst);
        r
    }
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
