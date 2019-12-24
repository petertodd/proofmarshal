//! Blobs and blob validation.

use core::any::type_name;
use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::{self, Range};
use core::ptr::{self, NonNull};
use core::slice;

use crate::{
    load::ValidateBlob,
    pointee::Pointee,
};

mod cursor;
pub use self::cursor::Error;

pub trait BlobValidator<T: ?Sized + ValidateBlob> {
    type Ok;
    type Error;

    type StructValidator : StructValidator<T, Ok=Self::Ok, Error=Self::Error>;
    type EnumValidator : EnumValidator<T, Ok=Self::Ok, Error=Self::Error>;

    fn metadata(&self) -> T::Metadata;

    fn validate_struct(self) -> Self::StructValidator;
    fn validate_enum(self) -> (u8, Self::EnumValidator);

    unsafe fn validate_option<U: ValidateBlob, F>(self, f: F) -> Result<Self::Ok, Self::Error>
        where F: FnOnce(U::Error) -> T::Error;

    fn validate_bytes(self, f: impl for<'a> FnOnce(Blob<'a, T>) -> Result<ValidBlob<'a, T>, T::Error>)
        -> Result<Self::Ok, Self::Error>
    where T: ValidateBlob;
}

pub trait StructValidator<T: ?Sized + ValidateBlob> {
    type Ok;
    type Error;

    fn field<U: ValidateBlob, F>(&mut self, f: F) -> Result<ValidBlob<U>, Self::Error>
        where F: FnOnce(U::Error) -> T::Error;

    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error>;
}

pub trait EnumValidator<T: ?Sized + ValidateBlob> {
    type Ok;
    type Error;

    fn field<U: ValidateBlob, F>(&mut self, f: F) -> Result<ValidBlob<U>, Self::Error>
        where F: FnOnce(U::Error) -> T::Error;

    /// Asserts that the enum is valid.
    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error>;
}

/// Unverified bytes from a persistent zone.
pub struct Blob<'a, T: ?Sized + Pointee> {
    // *invariant* over 'a
    marker: PhantomData<fn(&'a [u8]) -> &'a T>,
    ptr: NonNull<u8>,

    /// The pointer metadata.
    pub metadata: T::Metadata,
}

pub struct ValidBlob<'a, T: ?Sized + Pointee>(Blob<'a, T>);

impl<'a, T: ?Sized + Pointee> Blob<'a, T> {
    /// Creates a new `Blob` from a slice and metadata.
    ///
    /// Returns `None` if the slice is the wrong size for the metadata.
    pub fn new(buf: &'a [u8], metadata: T::Metadata) -> Option<Self> {
        if buf.len() == T::layout(metadata).size() {
            unsafe { Some(Self::new_unchecked(buf, metadata)) }
        } else {
            None
        }
    }

    /// Creates a new `Blob` from a slice and metadata, without checking that the slice is the
    /// correct size.
    ///
    /// # Safety
    ///
    /// The slice must be the correct size.
    pub unsafe fn new_unchecked(buf: &'a [u8], metadata: T::Metadata) -> Self {
        assert_eq!(T::layout(metadata).align(), 1,
                   "{} needs alignment", type_name::<T>());
        Self {
            marker: PhantomData,
            ptr: NonNull::new(buf.as_ptr() as *mut u8).unwrap(),
            metadata,
        }
    }

    pub fn into_validator(self) -> impl BlobValidator<T, Ok=ValidBlob<'a, T>, Error=cursor::Error<T::Error>>
        where T: ValidateBlob
    {
        cursor::BlobCursor::from(self)
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
}

impl<'a, T: ?Sized + Pointee> ValidBlob<'a, T> {
    pub fn to_ref(self) -> &'a T {
        let inner = unsafe { &*T::make_fat_ptr(self.ptr.as_ptr() as *const (), self.metadata) };
        assert_eq!(T::layout(self.metadata), core::alloc::Layout::for_value(inner),
                   "<{} as Pointee>::layout() incorrectly implemented", type_name::<T>());
        inner
    }
}


impl<'a, T: ?Sized + Pointee> ops::Deref for Blob<'a, T> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.clone().into()
    }
}

unsafe impl<T: ?Sized + Pointee> Sync for Blob<'_,T> {}
unsafe impl<T: ?Sized + Pointee> Send for Blob<'_,T> {}

impl<'a, T: ?Sized + Pointee> From<Blob<'a, T>> for &'a [u8] {
    fn from(blob: Blob<'a, T>) -> &'a [u8] {
        // Safe because it's the only safe ways to create blobs ensure the size is correct.
        unsafe {
            slice::from_raw_parts(blob.ptr.as_ptr(), T::layout(blob.metadata).size())
        }
    }
}

impl<'a, T: ?Sized + Pointee> From<ValidBlob<'a, T>> for Blob<'a, T> {
    fn from(blob: ValidBlob<'a, T>) -> Blob<'a, T> {
        blob.0
    }
}

impl<T: ?Sized + Pointee> fmt::Debug for Blob<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("slice", &&self[..])
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee> ops::Deref for ValidBlob<'a, T> {
    type Target = Blob<'a, T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + Pointee> Clone for Blob<'a, T> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            ptr: self.ptr,
            metadata: self.metadata,
        }
    }
}
impl<'a, T: ?Sized + Pointee> Copy for Blob<'a, T> {}

impl<'a, T: ?Sized + Pointee> Clone for ValidBlob<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized + Pointee> Copy for ValidBlob<'a, T> {}

#[cfg(test)]
mod test {
}
