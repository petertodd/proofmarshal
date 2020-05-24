use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use super::*;

impl<Z, T> Decode<Z> for Option<T>
where T: Decode<Z>
{
    fn decode_blob<'a>(mut blob: BlobDecoder<Z, Self>) -> Self {
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
