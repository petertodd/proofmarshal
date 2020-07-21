//! Efficient, fixed-size, "blob of bytes" serialization.

use std::any::type_name;
use std::borrow::Borrow;
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::fmt;
use std::slice;

use owned::IntoOwned;

use crate::pointee::Pointee;
use crate::ptr::*;
use crate::load::*;

//pub mod impls;
//pub mod save;
//pub use self::save::*;

pub mod layout;
pub use self::layout::BlobLayout;

/// Blob validation for `?Sized` types.
pub unsafe trait ValidateBlob : Pointee {
    /// Error returned when a load fails (for whatever reason).
    type BlobError : std::error::Error + 'static + Send + Sync;

    fn try_blob_layout(metadata: Self::Metadata) -> Result<BlobLayout, Self::LayoutError>;

    fn blob_layout() -> BlobLayout
        where Self: Sized,
    {
        Self::try_blob_layout(Self::make_sized_metadata())
            .expect("layout should always succeed for sized types")
    }

    fn validate_blob<'a>(blob: Blob<'a, Self>, ignore_padding: bool) -> Result<ValidBlob<'a, Self>, Self::BlobError>;
}

/// A reference to an unverified byte blob, for a specific type.
pub struct Blob<'a, T: ?Sized + Pointee> {
    // invariant so ValidateBlob(Dyn) implementations can't play games over what blob they return
    marker: PhantomData<fn(&'a [u8]) -> &'a T>,
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

    pub fn validate_fields(self, ignore_padding: bool) -> ValidateFields<'a, T> {
        ValidateFields {
            blob: self,
            idx: 0,
            ignore_padding,
        }
    }
}

impl<'a, T: ?Sized + ValidateBlob> Blob<'a, T> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &'a [u8] {
        let layout = T::try_blob_layout(self.metadata)
                       .unwrap_or_else(|err| {
                           todo!()
                       });
        unsafe { slice::from_raw_parts(self.ptr, layout.size()) }
    }
}

impl<'a, T: ?Sized + Pointee> Deref for ValidBlob<'a, T> {
    type Target = Blob<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + ValidateBlob> fmt::Debug for Blob<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("metadata", &self.metadata)
            .field("bytes", &self.as_bytes())
            .finish()
    }
}

impl<'a, T: ?Sized + ValidateBlob> fmt::Debug for ValidBlob<'a, T> {
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
        if slice.len() == T::blob_layout().size() {
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

    pub fn decode_fields<'z>(self, zone: &'z <T::Ptr as Ptr>::BlobZone) -> DecodeFields<'a, 'z, T, T::Ptr>
        where T: Load
    {
        DecodeFields {
            blob: self,
            idx: 0,
            zone,
        }
    }
}

/// `Blob` field validator.
pub struct ValidateFields<'a, T: ?Sized + Pointee> {
    blob: Blob<'a, T>,
    idx: usize,
    ignore_padding: bool,
}

impl<'a, T: ?Sized + ValidateBlob> fmt::Debug for ValidateFields<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .field("ignore_padding", &self.ignore_padding)
            .finish()
    }
}

impl<'a, T: ?Sized + ValidateBlob> ValidateFields<'a, T> {
    #[inline(always)]
    pub fn validate_blob<F: ValidateBlob>(&mut self) -> Result<ValidBlob<'a, F>, F::BlobError> {
        F::validate_blob(self.field_blob::<F>(), self.ignore_padding)
    }

    #[inline(always)]
    pub fn field_blob<F: ValidateBlob>(&mut self) -> Blob<'a, F> {
        Blob {
            marker: PhantomData,
            ptr: self.field_bytes(F::blob_layout().size()).as_ptr(),
            metadata: F::make_sized_metadata(),
        }
    }

    #[inline(always)]
    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let blob_bytes = self.blob.as_bytes();
        let new_idx = self.idx + size;

        assert!(new_idx <= blob_bytes.len(), "out of range");

        let r = &blob_bytes[self.idx .. new_idx];
        self.idx = new_idx;
        r
    }

    #[inline(always)]
    pub unsafe fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob.assume_valid()
    }
}

/// `ValidBlob` field cursor.
pub struct DecodeFields<'a, 'z, T: ?Sized + Pointee, P: Ptr> {
    blob: ValidBlob<'a, T>,
    idx: usize,
    zone: &'z P::BlobZone,
}

impl<'a, 'z, P: Ptr, T: ?Sized + ValidateBlob> fmt::Debug for DecodeFields<'a, 'z, T, P>
where P::BlobZone: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("idx", &self.idx)
            .field("zone", &self.zone)
            .finish()
    }
}

impl<'a, 'z, P: Ptr, T: ?Sized + ValidateBlob> DecodeFields<'a, 'z, T, P> {
    #[inline(always)]
    pub unsafe fn decode_unchecked<F: 'a + DecodePtr<P>>(&mut self) -> F {
        let blob = self.field_blob::<F>();
        let blob = blob.assume_valid();
        F::decode_blob(blob, self.zone)
    }

    #[inline(always)]
    pub fn field_blob<F: ValidateBlob>(&mut self) -> Blob<'a, F> {
        Blob {
            marker: PhantomData,
            ptr: self.field_bytes(F::blob_layout().size()).as_ptr(),
            metadata: F::make_sized_metadata(),
        }
    }

    #[inline(always)]
    pub fn field_bytes(&mut self, size: usize) -> &'a [u8] {
        let blob_bytes = self.blob.as_bytes();
        let new_idx = self.idx + size;

        assert!(new_idx <= blob_bytes.len(), "out of range");

        let r = &blob_bytes[self.idx .. new_idx];
        self.idx = new_idx;
        r
    }

    #[inline(always)]
    pub fn finish(self) -> ValidBlob<'a, T> {
        assert_eq!(self.idx, self.blob.as_bytes().len());
        self.blob
    }
}
