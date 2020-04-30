//! Cryptographic commitments.

pub mod digest;
pub use self::digest::Digest;

/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit : Verbatim {
    type Committed;

    fn commit(&self) -> Digest<Self::Committed> {
        Digest::hash_verbatim(self)
    }
}

pub trait Verbatim {
    /// The length of the verbatim encoding.
    const LEN: usize;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error>;
}

pub trait WriteVerbatim : Sized {
    type Error;

    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;

    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for _ in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }

    fn write<T: ?Sized + Verbatim>(self, value: &T) -> Result<Self, Self::Error> {
        value.encode_verbatim(self)
    }

    fn finish(self) -> Result<Self, Self::Error> {
        Ok(self)
    }
}

impl WriteVerbatim for Vec<u8> {
    type Error = !;

    #[inline]
    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(src);
        Ok(self)
    }
}

impl WriteVerbatim for &'_ mut [u8] {
    type Error = !;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        assert!(src.len() <= self.len(), "overflow");
        let (dst, rest) = self.split_at_mut(src.len());
        dst.copy_from_slice(src);
        Ok(rest)
    }
}

impl WriteVerbatim for sha2::Sha256 {
    type Error = !;

    #[inline(always)]
    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        use sha2::Digest as _;
        self.input(src);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
}
