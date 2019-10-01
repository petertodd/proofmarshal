/// Cryptographic digests.

use core::alloc::Layout;
use core::convert::TryFrom;
use core::cmp;
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU128;

use std::io;

use verbatim::Verbatim;

use crate::prelude::*;

use crate::ptr::{Coerced, Type};

/// Typed 32-byte hash digest.
#[repr(packed)]
pub struct Digest<T=!> {
    marker: PhantomData<fn(T) -> ()>,
    nonzero: NonZeroU128,
    rest: u128,
}

impl<T> Digest<T> {
    #[inline(always)]
    pub unsafe fn new_unchecked(buf: [u8;32]) -> Self {
        mem::transmute(buf)
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8;32] {
        assert_eq!(Layout::new::<Self>(), Layout::new::<[u8;32]>());
        unsafe { &*(self as *const _ as *const _) }
    }

    pub fn cast<U>(&self) -> Digest<U> {
        let raw: [u8;32] = (*self).into();
        Digest::try_from(raw).unwrap()
    }

    pub fn hash_verbatim(value: &T) -> Self
        where T: Verbatim
    {
        let mut stack = [0u8;128];
        let mut heap;

        let mut buf = if T::LEN > stack.len() {
            heap = vec![0; T::LEN];
            &mut heap[..]
        } else {
            &mut stack[0..T::LEN]
        };

        value.encode(&mut buf[..], &mut ()).expect("writing to a buffer is infallible");

        Self::hash_verbatim_bytes(buf, T::NONZERO_NICHE)
    }

    pub fn hash_verbatim_bytes(verbatim: &[u8], nonzero_niche: bool) -> Self {
        let mut digest = [0;32];
        if verbatim.len() < 32 {
            digest[0] = 0xff;
            digest[1..1+verbatim.len()].copy_from_slice(verbatim);

            Self::try_from(digest)
                 .expect("digest to be non-null")

        } else if verbatim.len() == 32 && nonzero_niche {
            digest.copy_from_slice(verbatim);

            Self::try_from(digest)
                 .expect("digest to be non-null")
        } else {
            sha256_hash(verbatim).cast()
        }

    }
}

#[inline(never)]
fn sha256_hash(buf: &[u8]) -> Digest<()> {
    use sha2::Digest as _;
    let d = sha2::Sha256::digest(buf);

    let mut digest = [0u8;32];
    digest.copy_from_slice(&d[..]);

    Digest::<()>::try_from(digest)
         .expect("digest to be non-null")
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error(());

impl<T> TryFrom<[u8;32]> for Digest<T> {
    type Error = Error;

    #[inline(always)]
    fn try_from(raw: [u8;32]) -> Result<Self, Error> {
        if raw[0..16] == [0;16] {
            Err(Error(()))
        } else {
            Ok(unsafe { Self::new_unchecked(raw) })
        }
    }
}

impl<T> From<Digest<T>> for [u8;32] {
    #[inline(always)]
    fn from(digest: Digest<T>) -> [u8;32] {
        *digest.as_bytes()
    }
}


impl<T> Default for Digest<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::try_from([0xff;32]).unwrap()
    }
}

impl<T> Clone for Digest<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        unsafe {
            Self::new_unchecked(*self.as_bytes())
        }
    }
}
impl<T> Copy for Digest<T> {}

impl<T> AsRef<[u8;32]> for Digest<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8;32] {
        self.as_bytes()
    }
}

impl<T> fmt::Display for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.as_bytes() {
            write!(f, "{:x}", b)?;
        }
        Ok(())
    }
}

impl<T> fmt::Debug for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Digest<{}>", self)
    }
}

impl<T> hash::Hash for Digest<T> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

impl<T> PartialEq for Digest<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes().eq(other.as_bytes())
    }
}
impl<T> Eq for Digest<T> {}

impl<T> PartialOrd for Digest<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}

impl<T> Ord for Digest<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl<T, P> Coerced<P> for Digest<T> {
    type Coerced = Self;
}

impl<T: 'static, P> Type<P> for Digest<T> {
}

impl<T, P> Verbatim<P> for Digest<T> {
    type Error = Error;

    const LEN: usize = mem::size_of::<Digest>();
    const NONZERO_NICHE: bool = true;

    #[inline(always)]
    fn encode<W: io::Write>(&self, mut dst: W, _: &mut impl verbatim::PtrEncode<P>) -> Result<W, io::Error> {
        dst.write_all(self.as_bytes())?;
        Ok(dst)
    }

    #[inline(always)]
    fn decode(src: &[u8], _: &mut impl verbatim::PtrDecode<P>) -> Result<Self, Self::Error> {
        let mut buf = [0u8;32];
        buf.copy_from_slice(src);

        Self::try_from(buf)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use hex_literal::hex;

    #[test]
    fn digest_default() {
        let d: Digest<()> = Default::default();

        assert_eq!(d.as_bytes(), &[255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn hash_verbatim() {
        assert_eq!(Digest::hash_verbatim(&()).as_bytes(),
                   &hex!("ff00000000000000000000000000000000000000000000000000000000000000"));
        assert_eq!(Digest::hash_verbatim(&1u8).as_bytes(),
                   &hex!("ff01000000000000000000000000000000000000000000000000000000000000"));
        assert_eq!(Digest::hash_verbatim(&0x12345678_u32).as_bytes(),
                   &hex!("ff78563412000000000000000000000000000000000000000000000000000000"));

        let d = Digest::hash_verbatim(&0x12345678_u32);
        let d2: Digest<Digest<u32>> = Digest::hash_verbatim(&d);
        assert_eq!(d.as_bytes(), d2.as_bytes());

        assert_eq!(Digest::hash_verbatim(&[0x0123_4567_89ab_cdef_u64;3]).as_bytes(),
                   &hex!("ff efcd ab89 6745 2301
                             efcd ab89 6745 2301
                             efcd ab89 6745 2301
                             0000 0000 0000 00  "));

        assert_eq!(Digest::hash_verbatim(&[0xff_u8;32]).as_bytes(),
                   &hex!("af9613760f72635fbdb44a5a0a63c39f12af30f950a6ee5c971be188e89c4051"));
    }
}
