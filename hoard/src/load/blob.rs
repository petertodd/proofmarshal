use std::any::type_name;
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ops::Deref;
use std::fmt;
use std::slice;

use crate::pointee::Pointee;
use crate::ptr::Ptr;
use crate::refs::Ref;

use super::Decode;

pub struct Blob<'a, T: ?Sized + Pointee> {
    marker: PhantomData<&'a [u8]>,
    ptr: *const u8,
    metadata: T::Metadata,
}

#[repr(transparent)]
pub struct ValidBlob<'a, T: ?Sized + Pointee>(Blob<'a, T>);

pub trait ValidateBlob : Sized {
    type Error : 'static + std::error::Error + Send + Sync;

    const BLOB_LEN: usize;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

pub unsafe trait BlobLen : Pointee {
    fn try_blob_len(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;
}

pub trait ValidateBlobPtr : Pointee + BlobLen {
    type Error : 'static + std::error::Error + Send + Sync;

    fn validate_blob_ptr<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

unsafe impl<T: ValidateBlob> BlobLen for T {
    fn try_blob_len(_: Self::Metadata) -> Result<usize, !> {
        Ok(Self::BLOB_LEN)
    }
}

impl<T: ValidateBlob> ValidateBlobPtr for T {
    type Error = T::Error;
    fn validate_blob_ptr<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        Self::validate_blob(blob)
    }
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

    pub fn into_loader<'z, P: Ptr>(self, zone: P::Zone) -> BlobLoader<'a, T, P> {
        BlobLoader::new(self, zone)
    }
}

pub struct BlobValidator<'a, T: ?Sized + Pointee> {
    blob: Blob<'a, T>,
    idx: usize,
}

impl<'a, T: ?Sized + Pointee> From<Blob<'a, T>> for BlobValidator<'a, T> {
    fn from(blob: Blob<'a, T>) -> Self {
        Self { blob, idx: 0 }
    }
}

impl<'a, T: ?Sized + BlobLen> fmt::Debug for BlobValidator<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee> Deref for BlobValidator<'a, T> {
    type Target = Blob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.blob
    }
}

impl<'a, T: ?Sized + BlobLen> BlobValidator<'a, T> {
    pub fn field<U: ValidateBlob>(&mut self) -> Result<&mut Self, U::Error> {
        let buf = self.field_bytes(U::BLOB_LEN);
        let blob = Blob::<U>::try_from(buf).unwrap();
        U::validate_blob(blob.into())?;
        Ok(self)
    }

    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.as_bytes().get(self.idx .. self.idx + size)
                                    .expect("out of range");
        self.idx += size;
        r
    }

    pub unsafe fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob.assume_valid()
    }
}

pub struct BlobLoader<'a, T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<P>,
    zone: P::Zone,
    blob: ValidBlob<'a, T>,
    idx: usize,
}

impl<'a, T: ?Sized + BlobLen, P: Ptr> fmt::Debug for BlobLoader<'a, T, P>
where P::Zone: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("zone", &self.zone)
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee, P: Ptr> Deref for BlobLoader<'a, T, P> {
    type Target = ValidBlob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.blob
    }
}

impl<'a, T: ?Sized + Pointee, P: Ptr> BlobLoader<'a, T, P> {
    pub fn new(blob: ValidBlob<'a, T>, zone: P::Zone) -> Self {
        Self {
            marker: PhantomData,
            zone,
            blob,
            idx: 0,
        }
    }
}

impl<'a, T: ?Sized + BlobLen, P: Ptr> BlobLoader<'a, T, P> {
    pub unsafe fn decode_unchecked<F>(&mut self) -> F
        where F: Decode<P>
    {
        let blob = self.field_blob::<F>().assume_valid();
        F::decode_blob(BlobLoader::new(blob, self.zone))
    }

    pub unsafe fn load_unchecked<F>(&mut self) -> Ref<'a, F>
        where F: Decode<P>
    {
        let blob = self.field_blob::<F>().assume_valid();
        F::load_blob(BlobLoader::new(blob, self.zone))
    }

    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.as_bytes().get(self.idx .. self.idx + size)
                                    .expect("out of range");
        self.idx += size;
        r
    }

    pub fn field_blob<F: ValidateBlob>(&mut self) -> Blob<'a, F> {
        self.field_bytes(F::BLOB_LEN).try_into().unwrap()
    }

    pub fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob
    }
}
