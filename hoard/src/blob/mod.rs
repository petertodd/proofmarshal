use std::any::type_name;
use std::borrow::Borrow;
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::fmt;
use std::slice;

use crate::pointee::Pointee;

pub mod impls;

pub struct Blob<'a, T: ?Sized + Pointee> {
    marker: PhantomData<&'a [u8]>,
    ptr: *const u8,
    metadata: T::Metadata,
}

#[repr(transparent)]
pub struct ValidBlob<'a, T: ?Sized + Pointee>(Blob<'a, T>);

impl<'a, T: ?Sized + Pointee> AsRef<Blob<'a, T>> for ValidBlob<'a, T> {
    fn as_ref(&self) -> &Blob<'a, T> {
        &self.0
    }
}

impl<'a, T: ?Sized + Pointee> Borrow<Blob<'a, T>> for ValidBlob<'a, T> {
    fn borrow(&self) -> &Blob<'a, T> {
        self.as_ref()
    }
}

pub trait ValidateBlob : Sized {
    const BLOB_LEN: usize;
    type Error : 'static + std::error::Error;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

pub unsafe trait BlobLen : Pointee {
    fn try_blob_len(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;
}

unsafe impl<T: ValidateBlob> BlobLen for T {
    fn try_blob_len(_: ()) -> Result<usize, Self::LayoutError> {
        Ok(T::BLOB_LEN)
    }
}

pub trait ValidateBlobPtr : BlobLen {
    type Error : 'static + std::error::Error;

    fn validate_blob_ptr<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

impl<T: ValidateBlob> ValidateBlobPtr for T {
    type Error = T::Error;

    fn validate_blob_ptr<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        ValidateBlob::validate_blob(blob)
    }
}


pub struct BlobValidator<'a, T: ?Sized + BlobLen> {
    cursor: BlobCursor<'a, T>,
}

pub unsafe trait Persist {
}

impl<'a, T: ?Sized + Pointee> Blob<'a, T> {
    pub unsafe fn new_unchecked(slice: &'a [u8], metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            metadata,
            ptr: slice.as_ptr(),
        }
    }

    pub unsafe fn assume_valid(self) -> ValidBlob<'a, T> {
        ValidBlob(self)
    }
}

impl<'a, T: ?Sized + BlobLen> Blob<'a, T> {
    pub fn as_bytes(&self) -> &'a [u8] {
        let blob_len = T::try_blob_len(self.metadata).unwrap();
        unsafe { slice::from_raw_parts(self.ptr, blob_len) }
    }
}

impl<'a, T: ?Sized + Pointee> Deref for ValidBlob<'a, T> {
    type Target = Blob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + BlobLen> fmt::Debug for Blob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("bytes", &self.as_bytes())
            .finish()
    }
}

impl<'a, T: ?Sized + BlobLen> fmt::Debug for ValidBlob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("bytes", &self.as_bytes())
            .finish()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct TryFromSliceError;

impl<'a, T: ValidateBlob> TryFrom<&'a [u8]> for Blob<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() == T::BLOB_LEN {
            unsafe { Ok(Self::new_unchecked(slice, T::make_sized_metadata())) }
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<'a, T: ?Sized + Pointee> ValidBlob<'a, T> {
    pub fn as_value(&self) -> &'a T
        where T: Persist
    {
        unsafe {
            &*T::make_fat_ptr(self.0.ptr as *const _, self.0.metadata)
        }
    }
}

pub struct BlobCursor<'a, T: ?Sized + BlobLen, B = Blob<'a, T>> {
    marker: PhantomData<Blob<'a, T>>,
    blob: B,
    idx: usize,
}

impl<'a, T: ?Sized + BlobLen, B> BlobCursor<'a, T, B>
where B: Borrow<Blob<'a, T>>
{
    pub fn field_blob<F: ValidateBlob>(&mut self) -> Blob<'a, F> {
        self.field_bytes(F::BLOB_LEN)
            .try_into().unwrap()
    }

    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.borrow()
                    .as_bytes().get(self.idx .. self.idx + size)
                               .expect("out of range");
        self.idx += size;
        r
    }

    pub fn finish(self) -> B {
        assert_eq!(self.idx, self.blob.borrow().as_bytes().len());
        self.into_inner()
    }

    pub fn into_inner(self) -> B {
        self.blob
    }
}

impl<'a, T: ?Sized + BlobLen, B> fmt::Debug for BlobCursor<'a, T, B>
where B: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .finish()
    }
}

impl<'a, T: ?Sized + BlobLen> From<ValidBlob<'a, T>> for Blob<'a, T> {
    fn from(blob: ValidBlob<'a, T>) -> Self {
        blob.0
    }
}

impl<'a, T: ?Sized + BlobLen, B> From<B> for BlobCursor<'a, T, B> {
    fn from(blob: B) -> Self {
        Self {
            marker: PhantomData,
            blob,
            idx: 0,
        }
    }
}

impl<'a, T: ?Sized + BlobLen> From<Blob<'a, T>> for BlobValidator<'a, T> {
    fn from(blob: Blob<'a, T>) -> Self {
        Self {
            cursor: blob.into(),
        }
    }
}

impl<'a, T: ?Sized + BlobLen> From<BlobValidator<'a, T>> for Blob<'a, T> {
    fn from(validator: BlobValidator<'a, T>) -> Self {
        validator.cursor.blob
    }
}

impl<'a, T: ?Sized + BlobLen> From<BlobCursor<'a, T>> for BlobValidator<'a, T> {
    fn from(cursor: BlobCursor<'a, T>) -> Self {
        Self { cursor }
    }
}

impl<'a, T: ?Sized + BlobLen> Deref for BlobValidator<'a, T> {
    type Target = BlobCursor<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.cursor
    }
}

impl<'a, T: ?Sized + BlobLen> DerefMut for BlobValidator<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cursor
    }
}

impl<'a, T: ?Sized + BlobLen> BlobValidator<'a, T> {
    pub fn field<U: ValidateBlob>(&mut self) -> Result<ValidBlob<'a, U>, U::Error> {
        let buf = self.field_bytes(U::BLOB_LEN);
        let blob = Blob::<U>::try_from(buf).unwrap();
        U::validate_blob(blob.into())
    }

    pub unsafe fn finish(self) -> ValidBlob<'a, T> {
        self.cursor.finish()
            .assume_valid()
    }
}
