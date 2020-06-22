//! Efficient, fixed-size, "blob of bytes" serialization.

use std::any::type_name;
use std::borrow::Borrow;
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::fmt;
use std::slice;

use crate::pointee::Pointee;
use crate::load::*;
use crate::zone::*;

//pub mod impls;
//pub mod save;
//pub use self::save::*;

pub mod layout;
pub use self::layout::BlobLayout;

pub mod padding;

/// Defines the size and layout of a blob of this type.
pub trait BlobSize : Sized {
    const BLOB_LAYOUT: BlobLayout;
}

/// `BlobSize`, but for `?Sized` types allowing the layout to depend on type metadata.
pub unsafe trait BlobSizeDyn : Pointee {
    fn try_blob_layout(metadata: Self::Metadata) -> Result<BlobLayout, Self::LayoutError>;
}

unsafe impl<T: BlobSize> BlobSizeDyn for T {
    fn try_blob_layout(_: ()) -> Result<BlobLayout, Self::LayoutError> {
        Ok(T::BLOB_LAYOUT)
    }
}

/// Blob validation for `Sized` types.
pub trait ValidateBlob<PaddingValidator> : BlobSize {
    /// Error returned when a load fails (for whatever reason).
    type Error : std::error::Error + 'static + Send + Sync;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: PaddingValidator) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

/// Blob validation for `?Sized` types.
pub trait ValidateBlobDyn<PaddingValidator> : BlobSizeDyn {
    /// Error returned when a load fails (for whatever reason).
    type Error : std::error::Error + 'static + Send + Sync;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: PaddingValidator) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

impl<V, T: ValidateBlob<V>> ValidateBlobDyn<V> for T {
    type Error = T::Error;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        T::validate_blob(blob, padval)
    }
}

/// A reference to an unverified byte blob, for a specific type.
pub struct Blob<'a, T: ?Sized + Pointee> {
    // invariant so ValidateBlob(Dyn) implementations can't play games over what blob they return
    marker: PhantomData<fn(&'a [u8]) -> &'a [u8]>,
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

    pub fn validate_fields<V>(self, padval: V) -> ValidateFields<'a, T, V> {
        ValidateFields {
            blob: self,
            idx: 0,
            padval,
        }
    }
}

impl<'a, T: ?Sized + BlobSizeDyn> Blob<'a, T> {
    pub fn as_bytes(&self) -> &'a [u8] {
        let layout = T::try_blob_layout(self.metadata).unwrap();
        unsafe { slice::from_raw_parts(self.ptr, layout.size()) }
    }
}

impl<'a, T: ?Sized + Pointee> Deref for ValidBlob<'a, T> {
    type Target = Blob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + BlobSizeDyn> fmt::Debug for Blob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("bytes", &self.as_bytes())
            .finish()
    }
}

impl<'a, T: ?Sized + BlobSizeDyn> fmt::Debug for ValidBlob<'a, T> {
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

impl<'a, T: BlobSize> TryFrom<&'a [u8]> for Blob<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() == T::BLOB_LAYOUT.size() {
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

    pub fn decode_fields<'z, Z>(self, zone: &'z Z) -> DecodeFields<'a, 'z, T, Z> {
        DecodeFields {
            blob: self,
            idx: 0,
            zone,
        }
    }
}

/// `Blob` field validator.
pub struct ValidateFields<'a, T: ?Sized + Pointee, V> {
    blob: Blob<'a, T>,
    idx: usize,
    padval: V,
}

impl<'a, T: ?Sized + BlobSizeDyn, V> fmt::Debug for ValidateFields<'a, T, V>
where V: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .field("padval", &self.padval)
            .finish()
    }
}

impl<'a, T: ?Sized + BlobSizeDyn, V: Copy> ValidateFields<'a, T, V> {
    pub fn validate_blob<F: ValidateBlob<V>>(&mut self) -> Result<ValidBlob<'a, F>, F::Error> {
        F::validate_blob(self.field_blob::<F>(), self.padval)
    }

    pub fn field_blob<F: BlobSize>(&mut self) -> Blob<'a, F> {
        self.field_bytes(F::BLOB_LAYOUT.size())
            .try_into().unwrap()
    }

    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.borrow()
                    .as_bytes().get(self.idx .. self.idx + size)
                               .expect("out of range");
        self.idx += size;
        r
    }

    pub unsafe fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob.assume_valid()
    }
}

/// `ValidBlob` field cursor.
pub struct DecodeFields<'a, 'z, T: ?Sized + Pointee, Z> {
    blob: ValidBlob<'a, T>,
    idx: usize,
    zone: &'z Z,
}

impl<'a, 'z, Z, T: ?Sized + BlobSizeDyn> fmt::Debug for DecodeFields<'a, 'z, T, Z>
where Z: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .field("zone", &self.zone)
            .finish()
    }
}

impl<'a, 'z, Z: Zone, T: ?Sized + BlobSizeDyn> DecodeFields<'a, 'z, T, Z> {
    pub unsafe fn decode_unchecked<F: Decode>(&mut self) -> F
        where Z: AsZone<F::Zone>,
    {
        let blob = self.field_blob::<F>();
        let blob = blob.assume_valid();
        F::decode_blob(blob, self.zone.as_zone())
    }

    pub fn field_blob<F: BlobSize>(&mut self) -> Blob<'a, F> {
        self.field_bytes(F::BLOB_LAYOUT.size())
            .try_into().unwrap()
    }

    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let r = self.blob.as_bytes().get(self.idx .. self.idx + size)
                                    .expect("out of range");
        self.idx += size;
        r
    }

    pub fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob
    }
}
