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

pub trait BlobZone : Sized {
    type BlobPtr : Ptr + Persist + Decode<Self>;
}

impl BlobZone for ! {
    type BlobPtr = !;
}

#[derive(Debug, PartialEq, Eq)]
pub struct PaddingError(());

pub struct Blob<'a, T: ?Sized + Pointee, Z> {
    marker: PhantomData<(fn() -> &'a T, fn() -> Z)>,
    ptr: *const u8,
    metadata: T::Metadata,
}

pub struct ValidBlob<'a, T: ?Sized + Pointee, Z>(Blob<'a, T, Z>);

pub struct FullyValidBlob<'a, T: ?Sized + Pointee, Z>(ValidBlob<'a, T, Z>);

pub struct BlobValidator<'a, T: ?Sized + Load<Z>, Z> {
    blob: ValidBlob<'a, T, Z>,
    state: T::ValidateChildren,
}

impl<'a, T: ?Sized + Pointee, Z> Clone for Blob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            ptr: self.ptr,
            metadata: self.metadata,
        }
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for Blob<'a, T, Z> {}

impl<T: ?Sized + Save<Z>, Z: BlobZone> ops::Deref for Blob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: ?Sized + Save<Z>, Z: BlobZone> ops::Deref for ValidBlob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl<T: ?Sized + Save<Z>, Z: BlobZone> ops::Deref for FullyValidBlob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl<'a, T: ?Sized + Pointee, Z: BlobZone> Blob<'a, T, Z> {
    pub fn new(buf: &'a [u8], metadata: T::Metadata) -> Option<Self>
        where T: Save<Z>
    {
        if buf.len() == T::dyn_blob_layout(metadata).size() {
            unsafe { Some(Self::new_unchecked(buf, metadata)) }
        } else {
            None
        }
    }

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

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> Blob<'a, T, Z> {
    pub fn validate_struct(self) -> ValidateFields<'a, T, Z> {
        ValidateFields(
            BlobCursor {
                blob: self,
                offset: 0,
            }
        )
    }

    pub fn validate_enum(self) -> (u8, ValidateVariant<'a, T, Z>) {
        (self[0],
         ValidateVariant(
             BlobCursor {
                 blob: self,
                 offset: 1,
             })
        )
    }

    pub fn assume_valid(self, state: T::ValidateChildren) -> BlobValidator<'a, T, Z> {
        BlobValidator {
            blob: ValidBlob(self),
            state,
        }
    }

    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a, T, Z> {
        ValidBlob(self).assume_fully_valid()
    }
}

impl<'a, T: ?Sized + Save<Z>, Z: BlobZone> Blob<'a, T, Z> {
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

impl<'a, T: Load<Z>, Z: Zone> TryFrom<&'a [u8]> for Blob<'a, T, Z> {
    type Error = TryFromBlobError;
    fn try_from(buf: &'a [u8]) -> Result<Self, TryFromBlobError> {
        Self::new(buf, T::make_sized_metadata())
             .ok_or(TryFromBlobError(()))
    }
}
*/

impl<T: ?Sized + Pointee, Z> fmt::Debug for Blob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.ptr)
            .field("metadata", &self.metadata)
            .finish()
    }
}



pub struct BlobCursor<'a, T: ?Sized + Pointee, Z> {
    blob: Blob<'a, T, Z>,
    offset: usize,
}

impl<T: ?Sized + Load<Z>, Z: BlobZone> fmt::Debug for BlobCursor<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("buf", &&self.blob[..])
            .field("metadata", &self.blob.metadata)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> BlobCursor<'a, T, Z> {
    pub fn field_blob<F: Decode<Z>>(&mut self) -> Blob<'a, F, Z> {
        let size = F::blob_layout().size();

        let start = self.offset;
        self.offset += size;

        let buf = self.blob.as_bytes().get(start .. self.offset)
                                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }

    fn validate_field<F: 'a + Decode<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }

    fn primitive_field<F: 'a + Primitive>(&mut self) -> Result<FullyValidBlob<'a, F, Z>, F::Error> {
        let blob = self.field_blob::<F>();
        F::validate_blob(blob)
    }
}

pub struct ValidateFields<'a, T: ?Sized + Pointee, Z>(BlobCursor<'a,T,Z>);

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> ValidateFields<'a, T, Z> {
    pub fn field_blob<F: Decode<Z>>(&mut self) -> Blob<'a, F, Z> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Decode<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }

    pub fn done(self, state: T::ValidateChildren) -> BlobValidator<'a, T, Z> {
        assert_eq!(self.0.offset, self.0.blob.len(),
                   "not fully validated");

        self.0.blob.assume_valid(state)
    }
}

pub struct ValidateVariant<'a, T: ?Sized + Pointee, Z>(BlobCursor<'a,T,Z>);

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> ValidateVariant<'a, T, Z> {
    pub fn field_blob<F: Decode<Z>>(&mut self) -> Blob<'a, F, Z> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Decode<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }

    pub fn done(self, state: T::ValidateChildren) -> Result<BlobValidator<'a, T, Z>, PaddingError> {
        if self.0.blob[self.0.offset .. ].iter().all(|b| *b == 0) {
            Ok(self.0.blob.assume_valid(state))
        } else {
            Err(PaddingError(()))
        }
    }
}

impl<'a, T: ?Sized + Pointee, Z> ValidBlob<'a,T,Z> {
    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a,T,Z> {
        FullyValidBlob(self)
    }

    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata
    }
}

impl<'a, T: ?Sized + Pointee, Z> Clone for ValidBlob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for ValidBlob<'a, T, Z> {}

impl<T: ?Sized + Pointee, Z> fmt::Debug for ValidBlob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.0.ptr)
            .field("metadata", &self.0.metadata)
            .finish()
    }
}

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> BlobValidator<'a, T, Z> {
    pub fn poll<V>(&mut self, validator: &mut V) -> Result<FullyValidBlob<'a, T, Z>, V::Error>
        where V: ValidatePtr<Z>
    {
        self.state.validate_children(validator)
            .map(|()|
                unsafe { self.blob.assume_fully_valid() }
            )
    }

    pub fn blob(&self) -> &ValidBlob<'a, T, Z> {
        &self.blob
    }

    pub fn state(&self) -> &T::ValidateChildren {
        &self.state
    }

    pub fn into_state(self) -> T::ValidateChildren {
        self.state
    }
}

impl<'a, T: ?Sized + Load<Z>, Z> fmt::Debug for BlobValidator<'a, T, Z>
where T::ValidateChildren: fmt::Debug,
      Z: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlobValidator")
            .field("blob", &self.blob)
            .field("state", &self.state)
            .finish()
    }
}


impl<T: ?Sized + Pointee, Z> fmt::Debug for FullyValidBlob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &(self.0).0.ptr)
            .field("metadata", &(self.0).0.metadata)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee, Z> FullyValidBlob<'a,T,Z> {
    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata()
    }
}

impl<'a, T: ?Sized + Pointee + Owned, Z> FullyValidBlob<'a,T,Z> {
    pub unsafe fn assume_valid(self) -> &'a T {
        &*T::make_fat_ptr((self.0).0.ptr as *const (), self.metadata())
    }

    pub unsafe fn assume_valid_ref(self) -> Ref<'a, T> {
        Ref::Borrowed(self.assume_valid())
    }
}

impl<'a, T: ?Sized + Pointee, Z> Clone for FullyValidBlob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for FullyValidBlob<'a, T, Z> {}

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone> FullyValidBlob<'a, T, Z> {
    pub fn decode_struct<L>(self, loader: L) -> FieldDecoder<'a,T,Z,L> {
        FieldDecoder {
             cursor: BlobCursor {
                 blob: (self.0).0,
                 offset: 0,
             },
            loader,
        }
    }

    pub fn decode_enum<L>(self, loader: L) -> (u8, FieldDecoder<'a,T,Z,L>) {
        (self[0],
         FieldDecoder {
             cursor: BlobCursor {
                 blob: (self.0).0,
                 offset: 1,
             },
            loader,
         })
    }
}

pub struct FieldDecoder<'a, T: ?Sized + Pointee, Z, L> {
    cursor: BlobCursor<'a, T, Z>,
    loader: L,
}

impl<'a, T: ?Sized + Load<Z>, Z: BlobZone, L> FieldDecoder<'a, T, Z, L>
where L: LoadPtr<Z>,
{
    pub fn field_blob<F: Decode<Z>>(&mut self) -> FullyValidBlob<'a, F, Z> {
        let blob = self.cursor.field_blob::<F>();

        unsafe { blob.assume_fully_valid() }
    }

    pub fn field<F: 'a + Decode<Z>>(&mut self) -> F {
        let blob = self.field_blob::<F>();

        F::decode_blob(blob, &self.loader)
          .take_sized()
    }
}

#[cfg(test)]
mod test {
}
