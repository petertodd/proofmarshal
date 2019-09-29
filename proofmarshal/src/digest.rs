/// Cryptographic digests.

use core::alloc::Layout;
use core::convert::TryFrom;
use core::cmp;
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU128;

/// Typed 32-byte hash digest.
#[repr(packed)]
pub struct Digest<T=!> {
    marker: PhantomData<fn(T) -> ()>,
    raw: [NonZeroU128;2],
}

impl<T> Digest<T> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8;32] {
        assert_eq!(Layout::new::<Self>(), Layout::new::<[u8;32]>());
        unsafe { &*(self as *const _ as *const _) }
    }

    pub fn cast<U>(&self) -> Digest<U> {
        let raw: [u8;32] = (*self).into();
        Digest::try_from(raw).unwrap()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error(());

impl<T> TryFrom<[u8;32]> for Digest<T> {
    type Error = Error;

    #[inline(always)]
    fn try_from(raw: [u8;32]) -> Result<Self, Error> {
        if raw[0..16] == [0;16] || raw[16..32] == [0;16] {
            Err(Error(()))
        } else {
            Ok(unsafe { mem::transmute(raw) })
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
        Self {
            marker: PhantomData,
            raw: self.raw,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_default() {
        let d: Digest<()> = Default::default();

        assert_eq!(d.as_bytes(), &[255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]);
    }
}
