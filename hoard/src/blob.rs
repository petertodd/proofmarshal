use std::any::type_name;
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ops::Deref;
use std::fmt;
use std::slice;

use crate::pointee::Pointee;
use crate::refs::Ref;
use crate::load::{Load, Decode};

pub trait ValidateBlob : Sized {
    type Error : std::error::Error;

    const BLOB_LEN: usize;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

pub unsafe trait BlobLen : Pointee {
    fn try_blob_len(metadata: Self::Metadata) -> Result<usize, Self::LayoutError>;
}

pub unsafe trait Persist {}

unsafe impl<T: ValidateBlob> BlobLen for T {
    fn try_blob_len(_: ()) -> Result<usize, Self::LayoutError> {
        Ok(Self::BLOB_LEN)
    }
}

pub struct Blob<'a, T: ?Sized + BlobLen> {
    marker: PhantomData<&'a [u8]>,
    metadata: T::Metadata,
    ptr: *const u8,
}

pub struct ValidBlob<'a, T: ?Sized + BlobLen>(Blob<'a, T>);

impl<'a, T: ?Sized + BlobLen> Deref for Blob<'a, T> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.bytes()
    }
}

impl<'a, T: ?Sized + BlobLen> Deref for ValidBlob<'a, T> {
    type Target = Blob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + BlobLen> fmt::Debug for Blob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("slice", &self.deref())
            .finish()
    }
}

impl<'a, T: ?Sized + BlobLen> fmt::Debug for ValidBlob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("slice", &self.deref())
            .finish()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct TryFromSliceError;

impl<'a, T: BlobLen> TryFrom<&'a [u8]> for Blob<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() == T::try_blob_len(T::make_sized_metadata()).unwrap() {
            unsafe { Ok(Self::new_unchecked(slice, T::make_sized_metadata())) }
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<'a, T: ?Sized + BlobLen> Blob<'a, T> {
    pub unsafe fn new_unchecked(slice: &'a [u8], metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            metadata,
            ptr: slice.as_ptr(),
        }
    }

    pub fn validate_fields(self) -> BlobValidator<'a, T> {
        BlobValidator {
            blob: self,
            idx: 0,
        }
    }

    pub fn bytes(&self) -> &'a [u8] {
        let blob_len = T::try_blob_len(self.metadata).unwrap();
        unsafe { slice::from_raw_parts(self.ptr, blob_len) }
    }

    pub unsafe fn assume_valid(self) -> ValidBlob<'a, T> {
        ValidBlob(self)
    }
}

impl<'a, T: ?Sized + BlobLen> ValidBlob<'a, T> {
    pub fn to_ref(self) -> &'a T
        where T: Persist
    {
        unsafe {
            &*T::make_fat_ptr(self.0.ptr as *const _, self.0.metadata)
        }
    }

    pub fn into_loader<'z, Z: ?Sized>(self, zone: &'z Z) -> BlobLoader<'a, 'z, T, Z> {
        BlobLoader::new(self, zone)
    }
}

pub struct BlobValidator<'a, T: ?Sized + BlobLen> {
    blob: Blob<'a, T>,
    idx: usize,
}

impl<'a, T: ?Sized + BlobLen> BlobValidator<'a, T> {
    pub fn validate_bytes<'b, F, R>(&'b mut self, size: usize, f: F) -> R
        where F: FnOnce(&'b [u8]) -> R
    {
        let subslice = self.blob.get(self.idx .. self.idx + size)
                                .expect("out of range");

        self.idx += size;
        f(subslice)
    }

    pub fn validate<'b, U: ValidateBlob>(&'b mut self) -> Result<ValidBlob<'b, U>, U::Error> {
        self.validate_bytes(U::BLOB_LEN, |slice| {
            let blob = Blob::<U>::try_from(slice).unwrap();
            U::validate_blob(blob)
        })
    }

    pub unsafe fn assume_valid(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.len());
        self.blob.assume_valid()
    }
}

pub struct BlobLoader<'a, 'z, T: ?Sized + BlobLen, Z: ?Sized> {
    blob: ValidBlob<'a, T>,
    zone: &'z Z,
    idx: usize,
}

impl<T: ?Sized + BlobLen, Z: ?Sized> fmt::Debug for BlobLoader<'_, '_, T, Z>
where T::Metadata: fmt::Debug,
      Z: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("zone", &self.zone)
            .field("idx", &self.idx)
            .finish()
    }
}

impl<'a, 'z, T: ?Sized + BlobLen, Z: ?Sized> BlobLoader<'a, 'z, T, Z> {
    pub fn new(blob: ValidBlob<'a, T>, zone: &'z Z) -> Self {
        Self {
            blob,
            zone,
            idx: 0,
        }
    }

    pub fn get_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.bytes().get(self.idx .. self.idx + size).expect("overflow");
        self.idx += size;
        r
    }

    pub fn get_blob<F: ValidateBlob>(&mut self) -> Blob<'a, F> {
        self.get_bytes(F::BLOB_LEN).try_into().unwrap()
    }

    pub unsafe fn load_unchecked<F>(&mut self) -> Ref<'a, F>
        where F: Decode<Z>
    {
        let blob = self.get_blob().assume_valid();
        F::load_blob(blob, self.zone)
    }

    pub unsafe fn decode_unchecked<F>(&mut self) -> F
        where F: Decode<Z>
    {
        let blob = self.get_blob().assume_valid();
        F::decode_blob(blob, self.zone)
    }

    pub fn assert_done(self) {
        assert_eq!(self.idx, self.blob.len());
    }

    pub fn to_ref(self) -> &'a T
        where T: Persist
    {
        self.blob.to_ref()
    }
}
