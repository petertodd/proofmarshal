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
    type Ptr : PtrBlob;
    type DecodeBytesError : 'static + std::error::Error;
    type ValidateError : 'static + std::error::Error;
    type ValidatePoll : ValidatePoll<Ptr = Self::Ptr, Error = Self::ValidateError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError>;

    fn validate_children(&self) -> Self::ValidatePoll;
}

pub trait Validator {
    type Ptr : PtrBlob;
    type Error : 'static + std::error::Error;

    fn check_blob<T: ?Sized + BlobDyn, F, R>(
        &mut self,
        ptr: Self::Ptr,
        metadata: T::Metadata,
        f: F
    ) -> Poll<Result<R, Self::Error>>
        where F: FnOnce(Option<Bytes<'_, T>>) -> R;
}

#[derive(Debug)]
#[repr(transparent)]
struct ValidatorAdapter<V: ?Sized, P> {
    marker: PhantomData<fn(P)>,
    inner: V,
}

impl<V: ?Sized, P> ValidatorAdapter<V, P> {
    fn new(r: &mut V) -> &mut Self {
        // SAFETY: repr(transparent)
        unsafe {
            &mut *(r as *mut V as *mut Self)
        }
    }
}

impl<V: ?Sized, P> Validator for ValidatorAdapter<V, P>
where V: Validator,
      P: PtrBlob,
      V::Ptr: FromPtr<P>,
{
    type Ptr = P;
    type Error = V::Error;

    fn check_blob<T: ?Sized + BlobDyn, F, R>(
        &mut self,
        ptr: Self::Ptr,
        metadata: T::Metadata,
        f: F
    ) -> Poll<Result<R, Self::Error>>
        where F: FnOnce(Option<Bytes<'_, T>>) -> R
    {
        self.inner.check_blob::<T, F, R>(
            ptr.into_ptr(),
            metadata,
            f
        )
    }
}

pub trait ValidatePoll {
    type Ptr : PtrBlob;
    type Error : 'static + std::error::Error;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = Self::Ptr>;

    fn validate_poll<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator,
              V::Ptr: FromPtr<Self::Ptr>
    {
        let adapted = ValidatorAdapter::<V, Self::Ptr>::new(validator);
        self.validate_poll_impl(adapted)
    }
}

impl ValidatePoll for () {
    type Ptr = !;
    type Error = !;

    fn validate_poll_impl<V>(&mut self, _: &mut V) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }
}

/// Dynamically sized blob.
pub unsafe trait BlobDyn : 'static + Pointee + IntoOwned {
    type Ptr : PtrBlob;
    type DecodeBytesError : 'static + std::error::Error;
    type ValidateError : 'static + std::error::Error;
    type ValidatePoll : ValidatePoll<Ptr = Self::Ptr, Error = Self::ValidateError>;

    fn try_size(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError>;

    fn validate_bytes_children(blob: Bytes<'_, Self>) -> Result<Self::ValidatePoll, Self::DecodeBytesError>;
}

unsafe impl<T: Blob> BlobDyn for T {
    type Ptr = T::Ptr;
    type DecodeBytesError = T::DecodeBytesError;
    type ValidateError = T::ValidateError;
    type ValidatePoll = T::ValidatePoll;

    fn try_size(_: ()) -> Result<usize, !> {
        Ok(Self::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        self.encode_bytes(dst)
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        T::decode_bytes(blob)
    }

    fn validate_bytes_children<'a>(blob: Bytes<'a, Self>) -> Result<Self::ValidatePoll, Self::DecodeBytesError> {
        let this = T::decode_bytes(blob)?;
        Ok(this.trust().validate_children())
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
