//! SHA256 support.

use std::convert::TryFrom;

use hoard::blob::{Bytes, BytesUninit};
use hoard::primitive::Primitive;

use super::{Digest, Hasher};

/// A SHA256 digest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Sha256Digest([u8; 32]);

impl AsRef<[u8]> for Sha256Digest {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Sha256Digest {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

/// A `Hasher` to create SHA256 digests.
#[derive(Default)]
pub struct Sha256Hasher(sha2::Sha256);

impl Digest for Sha256Digest {
    type Hasher = Sha256Hasher;
}

impl Hasher for Sha256Hasher {
    type Output = Sha256Digest;

    #[inline]
    fn hash_bytes(&mut self, buf: &[u8]) {
        use sha2::Digest as _;

        self.0.update(buf);
    }

    fn finish(self) -> Self::Output {
        use sha2::Digest as _;
        let raw = self.0.finalize();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&raw[..]);
        Sha256Digest(digest)
    }
}

impl Primitive for Sha256Digest {
    const BLOB_SIZE: usize = 32;
    type DecodeBytesError = !;

    #[inline]
    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&self.0)
    }

    #[inline]
    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let digest = <[u8; 32]>::try_from(&*src).unwrap();
        Ok(Self(digest))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use hex_literal::hex;

    #[test]
    fn test() {
        let mut hasher = Sha256Hasher::default();

        hasher.hash_bytes(&[]);
        let digest = hasher.finish();
        assert_eq!(digest.0,
            hex!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        let mut hasher = Sha256Hasher::default();
        hasher.hash_bytes(b"Hello World!");
        let digest = hasher.finish();
        assert_eq!(digest.0,
            hex!("7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069")
        );
    }
}
