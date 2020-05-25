use super::*;

use std::num;
use std::mem;
use std::slice;

use leint::Le;

macro_rules! impl_encode {
    ($t:ty) => {
        impl<Q, R> Encode<Q, R> for $t {
            type EncodePoll = Self;

            fn init_encode(&self, _: &impl SavePtr) -> Self {
                *self
            }
        }

        impl<Q, R> SavePoll<Q, R> for $t {
            fn save_poll<D: SavePtr>(&mut self, dst: D) -> Result<D, D::Error> {
                Ok(dst)
            }
        }
    }
}

macro_rules! impl_encode_for_persist {
    ($( $t:ty, )+) => {$(
        impl_encode!($t);

        impl EncodeBlob for $t {
            const BLOB_LEN: usize = mem::size_of::<Self>();

            fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                let src = unsafe {
                    slice::from_raw_parts(self as *const _ as *const _, mem::size_of::<Self>())
                };
                dst.write_bytes(src)?
                   .done()
            }
        }
    )+}
}

impl_encode_for_persist! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

impl_encode!(bool);

impl EncodeBlob for bool {
    const BLOB_LEN: usize = mem::size_of::<Self>();
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        let src = if *self { [1] } else { [0] };
        dst.write_bytes(&src)?
           .done()
    }
}

macro_rules! impl_encode_for_int {
    ($( $t:ty, )+) => {$(
        impl_encode!($t);

        impl EncodeBlob for $t {
            const BLOB_LEN: usize = mem::size_of::<Self>();

            fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                dst.write_bytes(&self.to_le_bytes())?
                   .done()
            }
        }
    )+}
}

impl_encode_for_int! {
    u16, u32, u64, u128,
    i16, i32, i64, i128,
}
