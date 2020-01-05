use super::*;

use std::mem;
use std::num;
use std::slice;

use hoard::prelude::Le;

impl Verbatim for ! {
    const LEN: usize = 0;

    #[inline(always)]
    fn encode_verbatim<W: WriteVerbatim>(&self, _dst: W) -> Result<W, W::Error> {
        match *self {}
    }
}

impl Prune for ! {
    #[inline(always)]
    fn prune(&mut self) { match *self {} }
    #[inline(always)]
    fn fully_prune(&mut self) { match *self {} }
}

impl Verbatim for bool {
    const LEN: usize = 1;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write_bytes(&[if *self { 1 } else { 0 }])
    }
}

macro_rules! impl_verbatim {
    ($($t:ty,)+) => {$(
        impl Verbatim for $t {
            const LEN: usize = mem::size_of::<$t>();
            fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
                let src = unsafe { slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>()) };
                dst.write_bytes(src)
            }
        }
    )+}
}

impl_verbatim! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

macro_rules! impl_prune {
    ($($t:ty,)+) => {$(
        impl Prune for $t {
            #[inline(always)]
            fn prune(&mut self) {}

            #[inline(always)]
            fn fully_prune(&mut self) {}
        }
    )+}
}

impl_prune! {
    (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}
