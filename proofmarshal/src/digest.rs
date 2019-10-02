/// Cryptographic digests.

use std::alloc::Layout;
use std::cmp;
use std::convert::TryFrom;
use std::fmt;
use std::hash;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::num::NonZeroU128;

use crate::verbatim::{Verbatim, PtrEncode, PtrDecode};

/// Typed 32-byte hash digest.
#[repr(packed)]
pub struct Digest<T=!> {
    marker: PhantomData<fn(T) -> ()>,
    _nonzero: NonZeroU128,
    _rest: u128,
}

impl<T> Digest<T> {
    /// Unsafely creates without checking the non-zero invariant.
    #[inline(always)]
    pub unsafe fn new_unchecked(buf: [u8;32]) -> Self {
        mem::transmute(buf)
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8;32] {
        assert_eq!(Layout::new::<Self>(), Layout::new::<[u8;32]>());
        unsafe { &*(self as *const _ as *const _) }
    }

    #[inline(always)]
    pub fn to_bytes(self) -> [u8;32] {
        *self.as_bytes()
    }

    /// Casts to a different type.
    pub fn cast<U>(&self) -> Digest<U> {
        let raw: [u8;32] = (*self).into();
        Digest::try_from(raw).unwrap()
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

/// Returned when conversions to a `Digest` fail due to the non-zero requirement.
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

impl<T, P> Verbatim<P> for Digest<T> {
    type Error = Error;

    const LEN: usize = mem::size_of::<Digest>();
    const NONZERO_NICHE: bool = true;

    #[inline(always)]
    fn encode<W: io::Write>(&self, mut dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        dst.write_all(self.as_bytes())?;
        Ok(dst)
    }

    #[inline(always)]
    fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        let mut buf = [0u8;32];
        buf.copy_from_slice(src);

        Self::try_from(buf)
    }
}
