/// Cryptographic digests.

use std::any::type_name;
use std::cmp;
use std::fmt;
use std::hash;
use std::marker::PhantomData;

use super::*;

/// Typed 32-byte hash digest.
#[repr(transparent)]
pub struct Digest<T=!> {
    marker: PhantomData<fn(T) -> ()>,
    buf: [u8;32],
}

impl<T> From<[u8;32]> for Digest<T> {
    fn from(buf: [u8;32]) -> Self {
        Self::new(buf)
    }
}

impl<T> Verbatim for Digest<T> {
    const LEN: usize = 32;
    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write_bytes(&self.buf)
    }
}

impl<T: Commit> Commit for Digest<T> {
    type Committed = T::Committed;
    fn commit(&self) -> Digest<Self::Committed> {
        self.cast()
    }
}

impl<T> Digest<T> {
    #[inline(always)]
    pub fn new(buf: [u8;32]) -> Self {
        Self { marker: PhantomData, buf }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8;32] {
        &self.buf
    }

    #[inline(always)]
    pub fn to_bytes(self) -> [u8;32] {
        self.buf
    }

    /// Casts to a different type.
    pub fn cast<U>(&self) -> Digest<U> {
        Digest::new(self.buf)
    }

    pub fn hash_verbatim_bytes(verbatim: &[u8]) -> Self {
        let mut digest = [0;32];
        if verbatim.len() < 32 {
            digest[0] = 0xff;
            digest[1..1+verbatim.len()].copy_from_slice(verbatim);

            Self::from(digest)
        } else {
            sha256_hash(verbatim).cast()
        }
    }

    pub fn hash_verbatim<U: Verbatim>(value: &U) -> Self {
        let mut fixed_bytes = [0; 512];
        let mut vec_buf;

        let buf = if let Some(buf) = fixed_bytes.get_mut(0 .. U::LEN) {
            buf
        } else {
            vec_buf = vec![0; U::LEN];
            &mut vec_buf[..]
        };

        let rest = value.encode_verbatim(&mut buf[..]).unwrap();
        assert_eq!(rest.len(), 0);

        Self::hash_verbatim_bytes(buf)
    }
}

#[inline(never)]
fn sha256_hash(buf: &[u8]) -> Digest<()> {
    use sha2::Digest as _;
    let d = sha2::Sha256::digest(buf);

    let mut digest = [0u8;32];
    digest.copy_from_slice(&d[..]);
    digest.into()
}

impl<T> From<Digest<T>> for [u8;32] {
    #[inline(always)]
    fn from(digest: Digest<T>) -> [u8;32] {
        digest.buf
    }
}


impl<T> Default for Digest<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::from([0x00; 32])
    }
}

impl<T> Clone for Digest<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::new(self.buf)
    }
}
impl<T> Copy for Digest<T> {}

impl<T> AsRef<[u8;32]> for Digest<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8;32] {
        self.as_bytes()
    }
}

/// Display representation:
///
/// ```
/// # use proofmarshal::commit::Digest;
/// assert_eq!(format!("{}", Digest::<u8>::default()),
///            "00000000000000000000000000000000");
/// ```
impl<T> fmt::Display for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.as_bytes() {
            write!(f, "{:x}", b)?;
        }
        Ok(())
    }
}

/// Debug representation:
///
/// ```
/// # use proofmarshal::commit::Digest;
/// assert_eq!(format!("{:?}", Digest::<u8>::default()),
///            "Digest<u8>(00000000000000000000000000000000)");
/// ```
impl<T> fmt::Debug for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Digest<{}>({})", type_name::<T>(), self)
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
