use super::*;

use std::mem;
use std::slice;

/*
macro_rules! impl_commit {
    ($t:ty) => {
        impl Commit for $t {
            type Committed = Self;
        }
    }
}
*/

impl Verbatim for ! {
    const VERBATIM_LEN: usize = 0;

    fn encode_verbatim_in(&self, _: &mut impl WriteVerbatim) {
        match *self {}
    }
}

impl Verbatim for () {
    const VERBATIM_LEN: usize = 0;

    fn encode_verbatim_in(&self, _dst: &mut impl WriteVerbatim) {
    }
}

impl Verbatim for bool {
    const VERBATIM_LEN: usize = 1;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        dst.write_bytes(&[if *self { 1 } else { 0 }]);
    }
}

/*
impl_commit!(bool);

macro_rules! impl_commit_for_persist {
    ($($t:ty,)+) => {$(
	impl_commit!($t);

        impl Verbatim for $t {
            const LEN: usize = mem::size_of::<Self>();

            fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
                let src = unsafe { slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>()) };
                dst.write_bytes(src);
            }
        }
    )+}
}

impl_commit_for_persist! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

macro_rules! impl_commit_for_int {
    ($($t:ty,)+) => {$(
	impl_commit!($t);

        impl Verbatim for $t {
            const LEN: usize = mem::size_of::<Self>();

            fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
                dst.write_bytes(&self.to_le_bytes());
            }
        }
    )+}
}

impl_commit_for_int! {
    u16, u32, u64, u128,
    i16, i32, i64, i128,
}
*/
