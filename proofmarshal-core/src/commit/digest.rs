/*
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
*/

use std::any::type_name;
use std::cmp;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::mem;

use super::*;

/// Typed 32-byte hash digest.
#[repr(transparent)]
pub struct Digest<T = !> {
    marker: PhantomData<fn(&T)>,
    buf: [u8;32],
}

impl<T> From<[u8;32]> for Digest<T> {
    fn from(buf: [u8;32]) -> Self {
        Self::new(buf)
    }
}

impl<T> Digest<T> {
    pub const LEN: usize = 32;

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
    pub fn cast<U>(&self) -> Digest<U> {
        Digest::new(self.buf)
    }

    /*
    pub fn hash_verbatim(value: &T) -> Self
        where T: Verbatim
    {
        if T::LEN <= 32 {
            struct Cursor<'a> {
                dst: &'a mut [u8],
            }

            impl<'a> WriteVerbatim for Cursor<'a> {
                fn write_bytes(&mut self, src: &[u8]) {
                    assert!(src.len() <= self.dst.len(), "overflow");
                    let (dst, rest) = mem::take(&mut self.dst).split_at_mut(src.len());
                    dst.copy_from_slice(src);
                    self.dst = rest;
                }
            }

            let mut buf = [0; 32];
            let mut cursor = Cursor {
                dst: &mut buf[0 .. T::LEN],
            };

            value.encode_verbatim_in(&mut cursor);

            assert_eq!(cursor.dst.len(), 0, "not all bytes written");

            Digest::from(buf)
        } else {
            let mut hasher = CommitHasher::new();
            hasher.write(value);
            hasher.finalize().cast()
        }
    }
    */
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

/*
/// Display representation:
///
/// ```
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{}", Digest::hash_verbatim(&0x1234_abcd_u32)),
///            "cdab341200000000000000000000000000000000000000000000000000000000");
/// ```
*/
impl<T> fmt::Display for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::LowerHex>::fmt(self, f)
    }
}

/*
/// Upper case hex representation:
///
/// ```
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{:X}", Digest::hash_verbatim(&0x1234_abcd_u32)),
///            "CDAB341200000000000000000000000000000000000000000000000000000000");
/// ```
*/
impl<T> fmt::UpperHex for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.as_bytes() {
            write!(f, "{:02X}", b)?;
        }
        Ok(())
    }
}

/*
/// Lower case hex representation:
///
/// ```
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{:x}", Digest::hash_verbatim(&0x1234_abcd_u32)),
///            "cdab341200000000000000000000000000000000000000000000000000000000");
/// ```
*/
impl<T> fmt::LowerHex for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.as_bytes() {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

/*
/// Debug representation:
///
/// ```
/// # use proofmarshal_core::commit::Digest;
/// assert_eq!(format!("{:?}", Digest::hash_verbatim(&0x1234_abcd_u32)),
///            "Digest<u32>(cdab341200000000000000000000000000000000000000000000000000000000)");
/// ```
*/
impl<T> fmt::Debug for Digest<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Digest<{}>({})", type_name::<T>(), self)
    }
}

/*
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

impl<T: ?Sized> ValidateBlob for Digest<T> {
    type Error = !;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(Blob::from(blob).assume_valid()) }
    }
}

impl<Q: Ptr, T: ?Sized> Decode<Q> for Digest<T> {
    fn decode_blob(blob: hoard::load::BlobDecoder<Q, Self>) -> Self {
        blob.to_value().clone()
    }
}

unsafe impl<T: ?Sized> Persist for Digest<T> {}

impl<Q, R, T: ?Sized> Encode<Q, R> for Digest<T> {
    type EncodePoll = Self;

    fn init_encode(&self, _: &impl SavePtr) -> Self::EncodePoll {
        *self
    }
}

impl<Q, R, T: ?Sized> SavePoll<Q, R> for Digest<T> {
    fn save_poll<D: SavePtr>(&mut self, dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }
}

impl<T: ?Sized> EncodeBlob for Digest<T> {
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
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
*/
