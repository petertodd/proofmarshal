use core::any::type_name;
use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use super::*;

mod layout;
pub use self::layout::*;

mod writeblob;
pub use self::writeblob::*;

#[derive(Debug, PartialEq, Eq)]
pub struct PaddingError(());

/// A blob of bytes that could be a valid value in a blob zone.
pub struct Blob<'a, T: ?Sized + Pointee, P> {
    marker: PhantomData<(fn() -> &'a T, P)>,
    ptr: *const u8,
    metadata: T::Metadata,
}

/// Wrapper around `Blob` indicating that the blob itself is valid for the type.
///
/// Children of the blob however are not necessarily valid!
pub struct ValidBlob<'a, T: ?Sized + Pointee, P>(Blob<'a, T, P>);

/// Wrapper around `ValidBlob` indicating that all children are also valid.
pub struct FullyValidBlob<'a, T: ?Sized + Pointee, P>(ValidBlob<'a, T, P>);

pub struct BlobValidator<'a, T: ?Sized + Load<P>, P: Ptr> {
    blob: ValidBlob<'a, T, P>,
    state: T::ValidateChildren,
}

impl<'a, T: ?Sized + Pointee, P> From<ValidBlob<'a, T, P>> for Blob<'a, T, P> {
    fn from(valid_blob: ValidBlob<'a, T, P>) -> Self {
        valid_blob.0
    }
}

impl<'a, T: ?Sized + Pointee, P> From<FullyValidBlob<'a, T, P>> for Blob<'a, T, P> {
    fn from(fully_valid_blob: FullyValidBlob<'a, T, P>) -> Self {
        (fully_valid_blob.0).0
    }
}

impl<'a, T: ?Sized + Pointee, P> From<FullyValidBlob<'a, T, P>> for ValidBlob<'a, T, P> {
    fn from(fully_valid_blob: FullyValidBlob<'a, T, P>) -> Self {
        fully_valid_blob.0
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for Blob<'a, T, P> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            ptr: self.ptr,
            metadata: self.metadata,
        }
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for Blob<'a, T, P> {}

impl<T: ?Sized + Save<P>, P: Ptr> ops::Deref for Blob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: ?Sized + Save<P>, P: Ptr> ops::Deref for ValidBlob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl<T: ?Sized + Save<P>, P: Ptr> ops::Deref for FullyValidBlob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl<'a, T: ?Sized + Pointee, P: Ptr> Blob<'a, T, P> {
    pub fn new(buf: &'a [u8], metadata: T::Metadata) -> Option<Self>
        where T: Save<P>
    {
        if buf.len() == T::dyn_blob_layout(metadata).size() {
            unsafe { Some(Self::new_unchecked(buf, metadata)) }
        } else {
            None
        }
    }
}

impl<'a, T: ?Sized + Pointee, P> Blob<'a, T, P> {
    pub unsafe fn new_unchecked(buf: &'a [u8], metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr: buf.as_ptr(),
            metadata,
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> Blob<'a, T, P> {
    pub fn validate_struct(self) -> ValidateFields<'a, T, P> {
        ValidateFields(
            BlobCursor {
                blob: self,
                offset: 0,
            }
        )
    }

    pub fn validate_primitive_struct(self) -> ValidatePrimitiveFields<'a, T, P> {
        ValidatePrimitiveFields(
            BlobCursor {
                blob: self,
                offset: 0,
            }
        )
    }

    pub fn validate_enum(self) -> (u8, ValidateVariant<'a, T, P>) {
        (self[0],
         ValidateVariant(
             BlobCursor {
                 blob: self,
                 offset: 1,
             })
        )
    }

    pub fn assume_valid(self, state: T::ValidateChildren) -> BlobValidator<'a, T, P> {
        BlobValidator {
            blob: ValidBlob(self),
            state,
        }
    }

    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a, T, P> {
        ValidBlob(self).assume_fully_valid()
    }
}

impl<'a, T: ?Sized + Save<P>, P: Ptr> Blob<'a, T, P> {
    fn as_bytes(&self) -> &'a [u8] {
        unsafe {
            slice::from_raw_parts(self.ptr,
                                  T::dyn_blob_layout(self.metadata).size())
        }
    }
}


/*
#[derive(Debug)]
pub struct TryFromBlobError(());

impl<'a, T: Load<P>, P: Ptr> TryFrom<&'a [u8]> for Blob<'a, T, P> {
    type Error = TryFromBlobError;
    fn try_from(buf: &'a [u8]) -> Result<Self, TryFromBlobError> {
        Self::new(buf, T::make_sized_metadata())
             .ok_or(TryFromBlobError(()))
    }
}
*/

impl<T: ?Sized + Pointee, P> fmt::Debug for Blob<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.ptr)
            .field("metadata", &self.metadata)
            .finish()
    }
}



pub struct BlobCursor<'a, T: ?Sized + Pointee, P> {
    blob: Blob<'a, T, P>,
    offset: usize,
}

impl<T: ?Sized + Load<P>, P: Ptr> fmt::Debug for BlobCursor<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("buf", &&self.blob[..])
            .field("metadata", &self.blob.metadata)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> BlobCursor<'a, T, P> {
    pub fn field_blob<F: Decode<P>>(&mut self) -> Blob<'a, F, P> {
        let size = F::BLOB_LAYOUT.size();

        let start = self.offset;
        self.offset += size;

        let buf = self.blob.as_bytes().get(start .. self.offset)
                                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }

    fn validate_field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }

    fn primitive_field<F: 'a + Primitive>(&mut self) -> Result<&'a F, F::Error> {
        let blob = self.field_blob::<F>();
        let blob = F::validate_blob(blob)?;
        Ok(unsafe { blob.assume_valid() })
    }
}

pub struct ValidateFields<'a, T: ?Sized + Pointee, P>(BlobCursor<'a,T,P>);

impl<'a, T: ?Sized + Load<P>, P: Ptr> ValidateFields<'a, T, P> {
    pub fn field_blob<F: Decode<P>>(&mut self) -> Blob<'a, F, P> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }

    pub fn primitive_field<F: 'a + Primitive>(&mut self) -> Result<&'a F, F::Error> {
        self.0.primitive_field::<F>()
    }

    pub fn done(self, state: T::ValidateChildren) -> BlobValidator<'a, T, P> {
        assert_eq!(self.0.offset, self.0.blob.len(),
                   "not fully validated");

        self.0.blob.assume_valid(state)
    }
}

pub struct ValidateVariant<'a, T: ?Sized + Pointee, P>(BlobCursor<'a,T,P>);

impl<'a, T: ?Sized + Load<P>, P: Ptr> ValidateVariant<'a, T, P> {
    pub fn field_blob<F: Decode<P>>(&mut self) -> Blob<'a, F, P> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }

    pub fn done(self, state: T::ValidateChildren) -> Result<BlobValidator<'a, T, P>, PaddingError> {
        if self.0.blob[self.0.offset .. ].iter().all(|b| *b == 0) {
            Ok(self.0.blob.assume_valid(state))
        } else {
            Err(PaddingError(()))
        }
    }
}

pub struct ValidatePrimitiveFields<'a, T: ?Sized + Pointee, P>(BlobCursor<'a,T,P>);

impl<'a, T: ?Sized + Primitive, P: Ptr> ValidatePrimitiveFields<'a, T, P> {
    pub fn field_blob<F: Primitive>(&mut self) -> Blob<'a, F, P> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Primitive>(&mut self) -> Result<&'a F, F::Error> {
        self.0.primitive_field::<F>()
    }

    pub unsafe fn assume_done(self) -> FullyValidBlob<'a, T, P> {
        assert_eq!(self.0.offset, self.0.blob.len(),
                   "not fully validated");

        self.0.blob.assume_fully_valid()
    }
}

impl<'a, T: ?Sized + Pointee, P> ValidBlob<'a,T,P> {
    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a,T,P> {
        FullyValidBlob(self)
    }

    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for ValidBlob<'a, T, P> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for ValidBlob<'a, T, P> {}

impl<T: ?Sized + Pointee, P> fmt::Debug for ValidBlob<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.0.ptr)
            .field("metadata", &self.0.metadata)
            .finish()
    }
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> BlobValidator<'a, T, P> {
    pub fn poll<V>(&mut self, validator: &mut V) -> Result<FullyValidBlob<'a, T, P>, V::Error>
        where V: ValidatePtr<P>
    {
        self.state.validate_children(validator)
            .map(|()|
                unsafe { self.blob.assume_fully_valid() }
            )
    }

    pub fn blob(&self) -> &ValidBlob<'a, T, P> {
        &self.blob
    }

    pub fn state(&self) -> &T::ValidateChildren {
        &self.state
    }

    pub fn into_state(self) -> T::ValidateChildren {
        self.state
    }
}

impl<'a, T: ?Sized + Load<P>, P: Ptr> fmt::Debug for BlobValidator<'a, T, P>
where T::ValidateChildren: fmt::Debug,
      P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlobValidator")
            .field("blob", &self.blob)
            .field("state", &self.state)
            .finish()
    }
}


impl<T: ?Sized + Pointee, P> fmt::Debug for FullyValidBlob<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &(self.0).0.ptr)
            .field("metadata", &(self.0).0.metadata)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee, P> FullyValidBlob<'a,T,P> {
    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata()
    }
}

impl<'a, T: ?Sized + Pointee + Owned, P> FullyValidBlob<'a,T,P> {
    pub unsafe fn assume_valid(self) -> &'a T {
        assert_eq!(T::align(self.metadata()), 1);
        &*T::make_fat_ptr((self.0).0.ptr as *const (), self.metadata())
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for FullyValidBlob<'a, T, P> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for FullyValidBlob<'a, T, P> {}

#[cfg(test)]
mod test {
}
