//! Blobs and blob validation.

use core::any::type_name;
use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use crate::{
    zone::Ptr,
    marshal::de::{Load, PtrValidator, ChildValidator},
    pointee::{Pointee, MaybeDropped},
};

pub mod validate;
use self::validate::*;

mod writeblob;
pub use self::writeblob::*;

/// Unverified bytes from a persistent zone.
pub struct Blob<'a, T: ?Sized + Pointee> {
    // *invariant* over 'a
    marker: PhantomData<fn(&'a [u8]) -> &'a T>,
    ptr: *const u8,

    /// The pointer metadata.
    pub metadata: T::Metadata,
}


/// A `Blob<T>` that has been fully verified.
#[derive(Debug)]
pub struct ValidBlob<'a, T: ?Sized> {
    marker: PhantomData<fn(&'a T)>,
    inner: &'a T,
}


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
            ptr: buf.as_ptr(),
            metadata,
        }
    }

    /// Asserts that `Blob` is fully valid, converting it into a `ValidBlob`.
    ///
    /// # Safety
    ///
    /// `ValidBlob<'a, T>` derefs to `&'a T`, so you are asserting that the `Blob` is valid for all
    /// purposes.
    pub unsafe fn assume_valid(self) -> ValidBlob<'a, T> {
        let inner = &*T::make_fat_ptr(self.ptr as *const (), self.metadata);

        assert_eq!(T::layout(self.metadata), core::alloc::Layout::for_value(inner),
                   "<{} as Pointee>::layout() incorrectly implemented", type_name::<T>());

        ValidBlob { marker: PhantomData, inner }
    }

    pub fn validate_struct(self) -> StructValidator<'a, T> {
        StructValidator(BlobCursor::from(self))
    }

    pub fn validate_primitive_struct(self) -> PrimitiveStructValidator<'a, T> {
        PrimitiveStructValidator(BlobCursor::from(self))
    }

    pub fn validate_enum(self) -> (u8, VariantValidator<'a, T>) {
        (self[0],
         VariantValidator(
             BlobCursor {
                 blob: self,
                 offset: 1,
             })
        )
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
            slice::from_raw_parts(blob.ptr, T::layout(blob.metadata).size())
        }
    }
}

impl<'a, T: ?Sized + Pointee> From<ValidBlob<'a, T>> for Blob<'a, T> {
    fn from(blob: ValidBlob<'a, T>) -> Blob<'a, T> {
        // SAFETY: Uninit bytes would be an issue, except that ValidBlob's are guaranteed to have
        // come from [u8]'s via Blob.
        Blob {
            marker: PhantomData,
            metadata: T::metadata(blob.inner),
            ptr: blob.inner as *const T as *const u8,
        }
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

impl<'a, T: ?Sized> ops::Deref for ValidBlob<'a, T> {
    type Target = &'a T;
    fn deref(&self) -> &Self::Target {
        &self.inner
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

impl<'a, T: ?Sized> Clone for ValidBlob<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized> Copy for ValidBlob<'a, T> {}

pub struct BlobValidator<'a, T: ?Sized + Load<P>, P: Ptr> {
    blob: Blob<'a, T>,
    state: T::ChildValidator,
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> BlobValidator<'a, T, P> {
    /// Creates a new `BlobValidator`.
    ///
    /// # Safety
    ///
    /// The `state` must be correct for the `blob`.
    pub unsafe fn new(blob: Blob<'a, T>, state: T::ChildValidator) -> Self {
        Self { blob, state }
    }

    /// Validates the blob's children.
    pub fn poll<V>(&mut self, ptr_validator: &V) -> Result<ValidBlob<'a, T>, V::Error>
        where V: PtrValidator<P>
    {
        match self.state.poll(ptr_validator) {
            Err(e) => Err(e),

            // SAFETY: Load is an unsafe trait, with the contract that the child validator
            // returning Ok() means the blob is valid.
            Ok(()) => Ok(unsafe { self.blob.assume_valid() }),
        }
    }

    pub fn into_state(self) -> T::ChildValidator {
        self.state
    }
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> From<ValidBlob<'a, T>> for BlobValidator<'a, T, P>
where T::ChildValidator: Default
{
    /// This is safe, even though `BlobValidator::new()` is unsafe, because we're starting from a
    /// `ValidBlob` so regardless of what the state does we can't accidentally return something
    /// invalid.
    fn from(blob: ValidBlob<'a, T>) -> Self {
        unsafe { BlobValidator::new(blob.into(), Default::default()) }
    }
}

#[cfg(test)]
mod test {
}
