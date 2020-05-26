use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use super::*;

impl<Q: Ptr, T: Decode<Q>> Decode<Q> for Option<T> {
    fn decode_blob<'a>(mut blob: BlobDecoder<Q, Self>) -> Self {
        unsafe {
            match blob.field_unchecked::<u8>() {
                1 => Some(blob.field_unchecked::<T>()),
                x => {
                    debug_assert_eq!(x, 0);
                    None
                }
            }
        }
    }
}
