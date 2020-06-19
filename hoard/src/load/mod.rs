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
use crate::zone::{Zone, Ptr};

/// A *type* that can be loaded from a zone pointer.
pub trait Load<Z> :
    IntoOwned +
    ValidateBlob<padding::CheckPadding> + ValidateBlob<padding::IgnorePadding>
{
    fn decode_blob(blob: ValidBlob<Self>, zone: &Z) -> Self::Owned
        where Z: BlobZone;

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self>
        where Z: BlobZone
    {
        Ref::Owned(Self::decode_blob(blob, zone))
    }
}

/// `Load`, but for sized types.
pub trait Decode<Z> : Sized + Load<Z> + IntoOwned<Owned=Self> + BlobSize {
}

/*
pub struct BlobLoader<'a, 'z, Z: Zone, T: ?Sized + BlobSize> {
    cursor: BlobCursor<'a, T, ValidBlob<'a, T>>,
    zone: &'z Z::Persist,
}

impl<'a, 'z, Z: Zone, T: ?Sized + BlobSize> ops::Deref for BlobLoader<'a, 'z, Z, T> {
    type Target = BlobCursor<'a, T, ValidBlob<'a, T>>;

    fn deref(&self) -> &Self::Target {
        &self.cursor
    }
}

impl<'a, 'z, Z: Zone, T: ?Sized + BlobSize> ops::DerefMut for BlobLoader<'a, 'z, Z, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cursor
    }
}

impl<'a, 'z, Z: Zone, T: ?Sized + BlobSize> BlobLoader<'a, 'z, Z, T> {
    pub fn new(blob: ValidBlob<'a, T>, zone: &'z Z::Persist) -> Self {
        Self {
            cursor: blob.into(),
            zone
        }
    }

    pub unsafe fn field_unchecked<F: Decode<Z>>(&mut self) -> F {
        let blob = self.field_blob::<F>().assume_valid();
        F::decode_blob(BlobLoader::new(blob, self.zone))
    }

    pub fn zone(&self) -> &'z Z::Persist {
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
*/

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
