use std::error::Error;
use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};

use thiserror::Error;

use owned::IntoOwned;

use crate::pointee::Pointee;
pub use crate::blob::*;
use crate::refs::Ref;

pub trait Load<Z: ?Sized> : Pointee + IntoOwned + BlobLen {
    fn decode_blob_owned<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Self::Owned;

    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Self
        where Self: Sized
    {
        let owned = Self::decode_blob_owned(blob, zone);

        unsafe {
            let owned = ManuallyDrop::new(owned);
            assert_eq!(Layout::new::<Self>(), Layout::new::<Self::Owned>());
            mem::transmute_copy(&owned)
        }
    }

    fn load_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self> {
        Ref::Owned(Self::decode_blob_owned(blob, zone))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
