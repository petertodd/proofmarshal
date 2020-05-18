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
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Self::Owned;

    fn load_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Ref<'a, Self> {
        Ref::Owned(Self::decode_blob(blob, zone))
    }
}

pub trait Decode<Z: ?Sized> : Sized + Load<Z, Owned=Self> + ValidateBlob {
}

impl<Z: ?Sized, T> Decode<Z> for T
where T: Load<Z, Owned=Self> + ValidateBlob
{
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
