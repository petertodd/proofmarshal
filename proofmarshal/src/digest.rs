/// Cryptographic digests.

use core::convert::TryFrom;
use core::cmp;
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU128;
use core::slice;

use nonzero::NonZero;
use persist::{Persist, MaybeValid, UninitBytes, Le};

/// Typed hash digest.
#[repr(transparent)]
pub struct Digest<T: ?Sized, D = Sha256Digest> {
    marker: PhantomData<fn(T) -> ()>,
    raw: D,
}

// A type of cryptographic hash digest.
pub trait CryptDigest : 'static + Persist + NonZero + Default + Copy + Eq + Ord + hash::Hash + Send + Sync
{
    const NAME: &'static str;

    /// Create a new digest by hashing bytes.
    fn hash_bytes(buf: &[u8]) -> Self;
}

impl<T: ?Sized, D> Digest<T,D> {
    #[inline(always)]
    pub fn new(raw: D) -> Self {
        Self { marker: PhantomData, raw }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self as *const _ as *const u8,
                                  mem::size_of::<Self>())
        }
    }
}

impl<T: ?Sized, D: Default> Default for Digest<T,D> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized, D: Clone> Clone for Digest<T,D> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::new(self.raw.clone())
    }
}
impl<T: ?Sized, D: Copy> Copy for Digest<T,D> {}

impl<T: ?Sized, D: fmt::Debug> fmt::Debug for Digest<T,D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Digest<{:?}", self.raw)
    }
}

unsafe impl<T: ?Sized, D: NonZero> NonZero for Digest<T,D> {}

impl<T: ?Sized, D: hash::Hash> hash::Hash for Digest<T,D> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}

impl<T: ?Sized, D: PartialEq> PartialEq for Digest<T,D> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.raw.eq(&other.raw)
    }
}
impl<T: ?Sized, D: Eq> Eq for Digest<T,D> {}

impl<T: ?Sized, D: PartialOrd> PartialOrd for Digest<T,D> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.raw.partial_cmp(&other.raw)
    }
}

impl<T: ?Sized, D: Ord> Ord for Digest<T,D> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

unsafe impl<T: ?Sized, D: Persist> Persist for Digest<T,D> {
    type Error = D::Error;

    #[inline(always)]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        unsafe {
            Ok(maybe.validate_fields()
                    .field::<D>()?
                    .assume_valid())
        }
    }

    #[inline(always)]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        dst.write(&self.raw);
        dst.done()
    }
}

/// A SHA256 digest.
#[repr(transparent)]
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Sha256Digest([Le<NonZeroU128>; 2]);

impl CryptDigest for Sha256Digest {
    const NAME: &'static str = "sha256";

    fn hash_bytes(buf: &[u8]) -> Self {
        use sha2::digest::Digest;

        let result = sha2::Sha256::digest(buf);

        let mut r = [0u8;32];
        r.copy_from_slice(&result[..]);

        Sha256Digest::try_from(r).expect("SHA256 returned all zeros hash")
    }
}

impl Sha256Digest {
    #[inline(always)]
    pub unsafe fn new_unchecked(bytes: [u8;32]) -> Self {
        mem::transmute(bytes)
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8;32] {
        unsafe { &*(self as *const _ as *const _) }
    }
}

unsafe impl NonZero for Sha256Digest {}

impl AsRef<[u8;32]> for Sha256Digest {
    #[inline(always)]
    fn as_ref(&self) -> &[u8;32] {
        self.as_bytes()
    }
}

impl From<Sha256Digest> for [u8;32] {
    #[inline(always)]
    fn from(digest: Sha256Digest) -> [u8;32] {
        let mut r = [0;32];
        r.copy_from_slice(digest.as_bytes());
        r
    }
}

impl TryFrom<[u8;32]> for Sha256Digest {
    type Error = Sha256DigestValidateError;

    #[inline(always)]
    fn try_from(buf: [u8;32]) -> Result<Self, Self::Error> {
        if (buf[0..16] == [0;16]) || (buf[16..32] == [0;16]) {
            Err(Sha256DigestValidateError)
        } else {
            Ok(unsafe { Sha256Digest::new_unchecked(buf) })
        }
    }
}

impl Default for Sha256Digest {
    #[inline(always)]
    fn default() -> Self {
        Sha256Digest::try_from([0xff;32]).unwrap()
    }
}

impl fmt::Display for Sha256Digest {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.as_bytes() {
            write!(f, "{:x}", b)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Sha256Digest {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sha256Digest<{}>", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sha256DigestValidateError;

unsafe impl Persist for Sha256Digest {
    type Error = Sha256DigestValidateError;

    #[inline(always)]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        let mut buf = [0;32];

        buf.copy_from_slice(&maybe[..]);

        Sha256Digest::try_from(buf)?;

        unsafe { Ok(maybe.assume_valid()) }
    }

    #[inline(always)]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        dst.write(self.as_bytes());
        dst.done()
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

    #[test]
    fn sha256_digest() {
        let d = Sha256Digest::default();
        assert_eq!(d.as_bytes(), &[255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]);

        assert_eq!(d.to_string(), "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");

        let d = Sha256Digest::hash_bytes(b"");
        assert_eq!(d.to_string(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        assert_eq!(format!("{:?}", d), "Sha256Digest<e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855>");
    }

    #[test]
    fn sha256_digest_error() {
        let buf: [u8;32] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];

        assert_eq!(Sha256Digest::try_from(buf).unwrap_err(),
                   Sha256DigestValidateError);

        let buf: [u8;32] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1];
        assert_eq!(Sha256Digest::try_from(buf).unwrap_err(),
                   Sha256DigestValidateError);

        let buf: [u8;32] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,
                            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
        assert_eq!(Sha256Digest::try_from(buf).unwrap_err(),
                   Sha256DigestValidateError);

        let buf: [u8;32] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,
                            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1];
        assert_eq!(Sha256Digest::try_from(buf).unwrap().to_string(),
                   "00000000000000010000000000000001");
    }
}
