use std::convert::TryInto;
use std::mem;
use std::num;
use std::slice;

use thiserror::Error;

use leint::Le;

use super::*;

macro_rules! impl_decode_for_persist {
    ($($t:ty,)+) => {$(
        impl<Z> Decode<Z> for $t {
            fn decode_blob(blob: ValidBlob<Self>, _: &Z) -> Self {
                blob.as_value().clone()
            }
        }
    )+}
}

impl_decode_for_persist! {
    (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

macro_rules! impl_decode_for_int {
    ($($t:ty,)+) => {$(
        impl<Z> Decode<Z> for $t {
            fn decode_blob(blob: ValidBlob<Self>, _: &Z) -> Self {
                let buf = blob.as_bytes();
                Self::from_le_bytes(buf.try_into().unwrap())
            }
        }
    )+}
}

impl_decode_for_int! {
    u16, u32, u64, u128,
    i16, i32, i64, i128,
}
