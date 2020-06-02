//! Loading of data behind zone pointers.

use std::error::Error;
use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops;

use thiserror::Error;

use owned::IntoOwned;

use crate::pointee::Pointee;
use crate::refs::Ref;
use crate::blob::*;
use crate::zone::Ptr;

pub mod impls;

pub trait Decode<Q: Ptr> : ValidateBlob {
    fn decode_blob(blob: BlobDecoder<Q, Self>) -> Self;
}

/// A *type* that can be loaded from a zone pointer.
pub trait Load<Q: Ptr> : IntoOwned + ValidateBlobPtr {
    fn load_blob(blob: BlobDecoder<Q, Self>) -> Self::Owned;

    fn deref_blob<'a>(blob: BlobDecoder<'a, '_, Q, Self>) -> Ref<'a, Self> {
        Ref::Owned(Self::load_blob(blob))
    }
}

impl<Q: Ptr, T: Decode<Q>> Load<Q> for T {
    fn load_blob<'a>(blob: BlobDecoder<'a, '_, Q, Self>) -> Self {
        Self::decode_blob(blob)
    }
}

pub struct BlobDecoder<'a, 'z, Q: Ptr, T: ?Sized + BlobLen> {
    cursor: BlobCursor<'a, T, ValidBlob<'a, T>>,
    zone: &'z Q::PersistZone,
}

impl<'a, 'z, Q: Ptr, T: ?Sized + BlobLen> ops::Deref for BlobDecoder<'a, 'z, Q, T> {
    type Target = BlobCursor<'a, T, ValidBlob<'a, T>>;

    fn deref(&self) -> &Self::Target {
        &self.cursor
    }
}

impl<'a, 'z, Q: Ptr, T: ?Sized + BlobLen> ops::DerefMut for BlobDecoder<'a, 'z, Q, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cursor
    }
}

impl<'a, 'z, Q: Ptr, T: ?Sized + BlobLen> BlobDecoder<'a, 'z, Q, T> {
    pub fn new(blob: ValidBlob<'a, T>, zone: &'z Q::PersistZone) -> Self {
        Self {
            cursor: blob.into(),
            zone
        }
    }

    pub unsafe fn field_unchecked<F: Decode<Q>>(&mut self) -> F {
        let blob = self.field_blob::<F>().assume_valid();
        F::decode_blob(BlobDecoder::new(blob, self.zone))
    }

    pub fn zone(&self) -> &'z Q::PersistZone {
        self.zone
    }

    pub fn to_value(self) -> &'a T
        where T: Persist
    {
        self.cursor.into_inner().as_value()
    }

    pub fn finish(self) {
        self.cursor.finish();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
