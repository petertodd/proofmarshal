use super::*;

use std::num;
use std::mem;
use std::slice;

use leint::Le;

macro_rules! impl_encoded {
    ($( $t:ty, )+) => {$(
        impl<R> Encoded<R> for $t {
            type Encoded = Self;
        }
    )+}
}

impl_encoded! {
    (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    u16, u32, u64, u128,
    i16, i32, i64, i128,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

macro_rules! impl_encode_for_persist {
    ($( $t:ty, )+) => {$(
        impl<Q, R> Encode<'_, Q, R> for $t {
            type State = ();

            fn init_encode_state(&self) -> () {}
            fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
                where D: Dumper<Source=Q, Target=R>
            {
                Ok(dst)
            }

            fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
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

impl<Q, R> Encode<'_, Q, R> for bool {
    type State = ();

    fn init_encode_state(&self) -> () {}
    fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        let src = if *self { [1] } else { [0] };
        dst.write_bytes(&src)?
           .done()
    }
}

macro_rules! impl_encode_for_int {
    ($( $t:ty, )+) => {$(
        impl<Q, R> Encode<'_, Q, R> for $t {
            type State = ();

            fn init_encode_state(&self) -> () {}
            fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
                where D: Dumper<Source=Q, Target=R>
            {
                Ok(dst)
            }

            fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
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
