//! Blobs and blob validation.

// FIXME: add Persist requirements re: alignment

use std::any::type_name;
use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit, size_of};
use std::ops;
use std::ptr::{self, NonNull};
use std::slice;

use thiserror::Error;

pub mod padding;
pub use self::padding::PaddingValidator;

mod writeblob;
pub use self::writeblob::WriteBlob;

use crate::bytes::Bytes;

/*
use crate::{
    load::Persist,
    pointee::Pointee,
};

mod cursor;
pub use self::cursor::Error;

*/

/// Unverified bytes from a persistent zone.
#[repr(transparent)]
pub struct Blob<'a, T: ?Sized> {
    // *invariant* over 'a
    marker: PhantomData<fn(Self) -> &'a T>,
    ptr: *const T,
}

pub struct ValidBlob<'a, T: ?Sized>(Blob<'a, T>);

impl<'a, T: ?Sized> Clone for Blob<'a, T> {
    fn clone(&self) -> Self {
        Blob {
            marker: PhantomData,
            ptr: self.ptr,
        }
    }
}

impl<'a, T: ?Sized> Clone for ValidBlob<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> ops::Deref for Blob<'_, T> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.ptr as *const u8, size_of::<T>())
        }
    }
}

impl<'a, T: ?Sized> Blob<'a, T> {
    /// Creates a `Blob` from a reference.
    ///
    /// # Safety
    ///
    /// This is unsafe because `T` might contain uninitialized bytes.
    pub unsafe fn from_ref_unchecked(r: &'a T) -> Self {
        Self::from_ptr(r)
    }

    pub unsafe fn from_ptr(ptr: *const T) -> Self {
        Self { marker: PhantomData, ptr }
    }

    /// Asserts that `Blob` is fully valid, converting it into a `ValidBlob`.
    ///
    /// # Safety
    ///
    /// `ValidBlob<'a, T>` derefs to `&'a T`, so you are asserting that the `Blob` is valid for all
    /// purposes.
    pub unsafe fn assume_valid(self) -> ValidBlob<'a, T> {
        ValidBlob(self)
    }

    pub fn into_cursor(self) -> BlobCursor<'a, T, padding::CheckPadding> {
        BlobCursor::new(self, padding::CheckPadding)
    }

    pub fn into_cursor_ignore_padding(self) -> BlobCursor<'a, T, padding::IgnorePadding> {
        BlobCursor::new(self, padding::IgnorePadding)
    }
}

impl<'a, T> From<&'a Bytes<T>> for Blob<'a, T> {
    /// Creates a `Blob` from a `Bytes` reference.
    ///
    /// This is safe because all bytes in `Blob` are initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::bytes::Bytes;
    /// # use hoard::marshal::blob::Blob;
    /// let bytes = Bytes::<bool>::new();
    /// let blob = Blob::from(&bytes);
    /// ```
    fn from(bytes: &'a Bytes<T>) -> Self {
        unsafe {
            Self::from_ptr(bytes.as_ptr() as *const T)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlobError<E, P> {
    Error(E),
    Padding(P),
}

impl<E, P> From<E> for BlobError<E, P> {
    fn from(err: E) -> Self {
        BlobError::Error(err)
    }
}

impl<E,P> BlobError<E,P> {
    pub fn map<F>(self, f: impl FnOnce(E) -> F) -> BlobError<F, P> {
        match self {
            BlobError::Padding(p) => BlobError::Padding(p),
            BlobError::Error(e) => BlobError::Error(f(e)),
        }
    }
}

pub struct BlobCursor<'a, T: ?Sized, P> {
    padding_validator: P,
    blob: Blob<'a, T>,
    offset: usize,
}

impl<'a, T: ?Sized, P: Clone> Clone for BlobCursor<'a, T, P> {
    fn clone(&self) -> Self {
        Self {
            padding_validator: self.padding_validator.clone(),
            blob: self.blob.clone(),
            offset: self.offset,
        }
    }
}

impl<'a, T, P> ops::Deref for BlobCursor<'a, T, P> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.blob
    }
}

impl<'a, T: ?Sized, P> BlobCursor<'a, T, P> {
    fn new(blob: Blob<'a, T>, padding_validator: P) -> Self {
        Self { padding_validator, blob, offset: 0 }
    }

    pub unsafe fn cast_unchecked<U>(self) -> BlobCursor<'a, U, P> {
        BlobCursor {
            padding_validator: self.padding_validator,
            blob: self.blob.cast_unchecked(),
            offset: self.offset
        }
    }
}

pub trait ValidateBlob {
    type Error : 'static + std::error::Error + Send + Sync;
    fn validate<'a, V>(blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator;
}

impl<'a, T: ValidateBlob, V: PaddingValidator> BlobCursor<'a, T, V> {
    pub fn field<U: ValidateBlob, F>(&mut self, f: F) -> Result<ValidBlob<'a, U>, BlobError<T::Error, V::Error>>
        where F: FnOnce(U::Error) -> T::Error
    {
        unsafe {
            self.field_unchecked::<U,F>(mem::size_of::<T>(), f)
        }
    }

    pub unsafe fn assume_valid(self) -> Result<ValidBlob<'a, T>, BlobError<T::Error, V::Error>> {
        Ok(self.blob.assume_valid())
    }

    pub unsafe fn validate_padding(self) -> Result<ValidBlob<'a, T>, BlobError<T::Error, V::Error>> {
        todo!()
    }

    pub fn validate_bytes(self, f: impl FnOnce(Blob<'a, T>) -> Result<ValidBlob<'a, T>, T::Error>)
        -> Result<ValidBlob<'a, T>, BlobError<T::Error, V::Error>>
    {
        f(self.blob).map_err(BlobError::Error)
    }
}

impl<'a, T: ?Sized + ValidateBlob, V: PaddingValidator> BlobCursor<'a, T, V> {
    unsafe fn field_unchecked<U: ValidateBlob, F>(&mut self, size: usize, f: F) -> Result<ValidBlob<'a, U>, BlobError<T::Error, V::Error>>
        where F: FnOnce(U::Error) -> T::Error
    {
        assert_eq!(mem::align_of::<U>(), 1);
        let field_ptr = self.blob.ptr.cast::<u8>()
                            .offset(self.offset as isize)
                            .cast::<U>();

        self.offset += mem::size_of::<U>();
        assert!(self.offset <= size, "overflow");

        let field = BlobCursor::new(Blob::from_ptr(field_ptr), self.padding_validator);

        match U::validate(field) {
            Ok(blob) => Ok(blob),
            Err(BlobError::Padding(p)) => Err(BlobError::Padding(p)),
            Err(BlobError::Error(u)) => Err(BlobError::Error(f(u))),
        }
    }
}

impl<'a, T: ?Sized> ValidBlob<'a, T> {
    pub fn to_ref(self) -> &'a T {
        unsafe { &*self.0.ptr }
    }
}

impl<T: ?Sized> fmt::Debug for Blob<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl<T: ?Sized> fmt::Debug for ValidBlob<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.0.ptr)
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[error("slice wrong size for blob")]
pub struct TryFromSliceError;

impl<'a, T> TryFrom<&'a [u8]> for Blob<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() == size_of::<T>() {
            assert_eq!(mem::align_of::<T>(), 1, "FIXME");
            Ok(unsafe { Blob::from_ptr(slice.as_ptr() as *const T) })
        } else {
            Err(TryFromSliceError)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// It *is* legal to have a Blob to the ! type, and doing things with that blob shouldn't cause
    /// issues.
    #[test]
    fn never_blob() {
        let empty: &[u8;0] = unsafe { &*NonNull::dangling().as_ptr() };
        let never_blob = Blob::<!>::try_from(&empty[..]).unwrap();
        assert_eq!(&never_blob[..], &[]);
        assert_eq!(format!("{:?}", never_blob),
                   "hoard::marshal::blob::Blob<!> { ptr: 0x1 }");
    }
}
