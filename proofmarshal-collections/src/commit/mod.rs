//! Cryptographic commitments.

pub mod digest;
pub use self::digest::Digest;

/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit {
    type Committed : 'static;

    fn commit(&self) -> Digest<Self::Committed>;
}

/// Verbatim encoding.
pub trait Verbatim {
    /// The length of the verbatim encoding.
    const LEN: usize;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error>;
}

pub trait WriteVerbatim : Sized {
    type Error;

    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;

    fn write<T: Verbatim>(self, value: &T) -> Result<Self, Self::Error> {
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

#[cfg(test)]
mod tests {
}
