use super::*;

use std::mem;
use std::slice;

impl Commit for ! {
    const VERBATIM_LEN: usize = 0;
    type Committed = Self;

    fn encode_verbatim(&self, _: &mut impl WriteVerbatim) {
        match *self {}
    }
}

impl Commit for () {
    const VERBATIM_LEN: usize = 0;
    type Committed = Self;

    fn encode_verbatim(&self, _dst: &mut impl WriteVerbatim) {
    }
}

impl Commit for bool {
    const VERBATIM_LEN: usize = 1;
    type Committed = Self;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write_bytes(&[if *self { 1 } else { 0 }]);
    }
}

macro_rules! impl_commit_for_int {
    ($($t:ty,)+) => {$(
        impl Commit for $t {
            type Committed = Self;
            const VERBATIM_LEN: usize = mem::size_of::<Self>();

            fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
                dst.write_bytes(&self.to_le_bytes());
            }
        }
    )+}
}

impl_commit_for_int! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(42u8.to_verbatim(), &[42]);
        assert_eq!(42u8.commit().as_bytes(), &[42, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);
    }
}
