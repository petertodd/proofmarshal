//! Cryptographic commitments.

use std::io;
use std::num;

use crate::digest::Digest;
use crate::verbatim::Verbatim;

/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit {
    type Committed : 'static + Verbatim<()>;

    fn commit(&self) -> Digest<Self::Committed> {
        let len = <Self::Committed as Verbatim<()>>::LEN;

        let mut stack = [0u8;128];
        let mut heap;

        let buf = if len > stack.len() {
            heap = vec![0; len];
            &mut heap[..]
        } else {
            &mut stack[0..len]
        };

        self.encode_commit_verbatim(&mut buf[..]).unwrap();
        Digest::hash_verbatim_bytes(buf, <Self::Committed as Verbatim<()>>::NONZERO_NICHE)
    }

    fn encode_commit_verbatim<W: io::Write>(&self, dst: W) -> Result<W, io::Error>;
}

impl<T: 'static> Commit for Digest<T> {
    type Committed = Self;

    fn commit(&self) -> Digest<Self::Committed> {
        self.cast()
    }

    fn encode_commit_verbatim<W: io::Write>(&self, mut dst: W) -> Result<W, io::Error> {
        dst.write_all(&self.to_bytes())?;
        Ok(dst)
    }
}

impl<T: Commit> Commit for &'_ T {
    type Committed = T::Committed;

    fn commit(&self) -> Digest<Self::Committed> {
        (**self).commit()
    }

    fn encode_commit_verbatim<W: io::Write>(&self, mut dst: W) -> Result<W, io::Error> {
        (**self).encode_commit_verbatim(dst)
    }
}

impl<T: Commit> Commit for &'_ mut T {
    type Committed = T::Committed;

    fn commit(&self) -> Digest<Self::Committed> {
        (**self).commit()
    }

    fn encode_commit_verbatim<W: io::Write>(&self, mut dst: W) -> Result<W, io::Error> {
        (**self).encode_commit_verbatim(dst)
    }
}

macro_rules! impl_primitives {
    ($( $t:ty, )+) => {
        $(
            impl Commit for $t {
                type Committed = $t;

                fn encode_commit_verbatim<W: io::Write>(&self, dst: W) -> Result<W, io::Error> {
                    <Self as Verbatim>::encode(self, dst, &mut ())
                }
            }
        )+
    }
}

impl_primitives! {
    !, (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
    num::NonZeroU8, num::NonZeroU16, num::NonZeroU32, num::NonZeroU64, num::NonZeroU128,
    num::NonZeroI8, num::NonZeroI16, num::NonZeroI32, num::NonZeroI64, num::NonZeroI128,
}


macro_rules! impl_arrays {
    ($($N:literal)+) => {
        $(
            impl<T: Commit> Commit for [T;$N] {
                type Committed = [T::Committed;$N];

                fn encode_commit_verbatim<W: io::Write>(&self, mut dst: W) -> Result<W, io::Error> {
                    for item in self {
                        dst = item.encode_commit_verbatim(dst)?;
                    }
                    Ok(dst)
                }
            }
        )+
    }
}

impl_arrays! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

#[cfg(test)]
mod tests {
    use super::*;

    use hex_literal::hex;

    #[test]
    fn commit_verbatim() {
        macro_rules! t {
            ($v:expr, $h:expr) => {{
                let v = $v;
                assert_eq!(v.commit().to_bytes(),
                           hex!($h));
            }}
        }

        t!((), "ff00000000000000000000000000000000000000000000000000000000000000");
        t!(1u8, "ff01000000000000000000000000000000000000000000000000000000000000");
        t!(0x12345678_u32,
           "ff78563412000000000000000000000000000000000000000000000000000000");
        t!([0x0123_4567_89ab_cdef_u64;3],
           "ff efcd ab89 6745 2301
               efcd ab89 6745 2301
               efcd ab89 6745 2301
               0000 0000 0000 00  ");
        t!([0xff_u8;32],
           "af9613760f72635fbdb44a5a0a63c39f12af30f950a6ee5c971be188e89c4051");
    }
}
