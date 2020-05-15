//! Cryptographic digests.
//!
//! # Variance
//!
//! A `Digest<T>` is *contravariant* with respect to `T`. That basically means the normal borrowing
//! rules are inverted, allowing the following to compile:
//!
//! ```
//! # use proofmarshal_core::commit::digest::Digest;
//! fn coerce<'a>(d: Digest<&'a u8>) -> Digest<&'static u8> {
//!     d
//! }
//! ```
//!
//! However the other way around doesn't compile:
//!
//! ```compile_fail
//! # use proofmarshal_core::commit::digest::Digest;
//! fn coerce<'a>(d: Digest<&'static u8>) -> Digest<&'a u8> {
//!     d
//! }
//! ```

use std::any::type_name;
use std::cmp;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::mem;

use hoard::load::*;
use hoard::save::*;
use hoard::primitive::*;

use super::*;

/// Typed 32-byte hash digest.
#[repr(transparent)]
pub struct Digest<T: ?Sized = !> {
    marker: PhantomData<fn(&T) -> Self>,
    buf: [u8;32],
}

impl<T: ?Sized> From<[u8;32]> for Digest<T> {
    fn from(buf: [u8;32]) -> Self {
        Self::new(buf)
    }
}

impl<T: ?Sized> Verbatim for Digest<T> {
    const LEN: usize = 32;

    #[inline(always)]
    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write_bytes(&self.buf)
    }
}

impl<T: ?Sized> Digest<T> {
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

    /// Casts the digest to a different type.
    pub fn cast<U: ?Sized>(&self) -> Digest<U> {
        Digest::new(self.buf)
    }

    pub fn hash_verbatim<U: ?Sized + Verbatim>(value: &U) -> Self {
        if U::LEN <= 32 {
            let mut buf = [0; 32];
            let rest = value.encode_verbatim(&mut buf[0 .. U::LEN]).unwrap();
            assert_eq!(rest.len(), 0, "some bytes remaining");

            Digest::from(buf)
        } else if U::LEN <= 1024 {
            let mut buf = [0; 1024];
            let buf = &mut buf[0 .. U::LEN];
            let rest = value.encode_verbatim(&mut buf[..]).unwrap();
            assert_eq!(rest.len(), 0);

            sha256_hash(buf).cast()
        } else {
            todo!()
        }
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

impl<T: ?Sized> From<Digest<T>> for [u8;32] {
    #[inline(always)]
    fn from(digest: Digest<T>) -> [u8;32] {
        digest.buf
    }
}


impl<T: ?Sized> Default for Digest<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::from([0x00; 32])
    }
}

impl<T: ?Sized> Clone for Digest<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::new(self.buf)
    }
}
impl<T: ?Sized> Copy for Digest<T> {}

impl<T: ?Sized> AsRef<[u8;32]> for Digest<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8;32] {
        self.as_bytes()
    }
}

/// Display representation:
///
/// ```
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{}", Digest::<u8>::default()),
///            "00000000000000000000000000000000");
/// ```
impl<T: ?Sized> fmt::Display for Digest<T> {
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
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{:?}", Digest::<u8>::default()),
///            "Digest<u8>(00000000000000000000000000000000)");
/// ```
impl<T: ?Sized> fmt::Debug for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Digest<{}>({})", type_name::<T>(), self)
    }
}

impl<T: ?Sized> hash::Hash for Digest<T> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

impl<T: ?Sized> PartialEq for Digest<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes().eq(other.as_bytes())
    }
}
impl<T: ?Sized> Eq for Digest<T> {}

impl<T: ?Sized> PartialOrd for Digest<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}

impl<T: ?Sized> Ord for Digest<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl<T: ?Sized> Load for Digest<T> {
    type Error = !;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, !> {
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<R, T: ?Sized> Saved<R> for Digest<T> {
    type Saved = Self;
}

impl<Q, R, T: ?Sized> Save<'_, Q, R> for Digest<T> {
    type State = ();

    fn init_save_state(&self) -> Self::State {}

    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut Self::State, dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        <Self as Save<Q,R>>::encode_blob(self, state, dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&self.buf)?
           .done()
    }
}

impl<T: ?Sized> Primitive for Digest<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_variance() {
        fn wants_static_digest<'a>(d: Digest<&'static u8>) -> Digest {
            d.cast()
        }

        fn has_nonstatic_digest<'a>(d: Digest<&'a u8>) -> Digest {
            wants_static_digest(d)
        }
    }
}
