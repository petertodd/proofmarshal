use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use super::*;

impl<Z, T: Decode<Z>> Decode<Z> for Option<T> {
    fn decode_blob<'a>(blob: ValidBlob<Self>, zone: &Z) -> Self {
        if let Some(niche) = T::BLOB_LAYOUT.niche() {
            todo!()
        } else {
            let mut fields = blob.decode_fields(zone);
            unsafe {
                match fields.decode_unchecked::<u8>() {
                    1 => Some(fields.decode_unchecked::<T>()),
                    x => {
                        debug_assert_eq!(x, 0);
                        None
                    }
                }
            }
        }
    }
}
