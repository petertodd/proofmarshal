//! Cryptographic commitments.

pub mod digest;
pub use self::digest::Digest;

/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit {
    fn commit(&self) -> Digest<Self>;
}

/// Verbatim encoding.
pub trait Verbatim {
    /// The length of the verbatim encoding.
    const LEN: usize;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error>;
}

/*
impl<T: Verbatim> Commit for T {
    fn commit(&self) -> Digest<Self> {
        let mut fixed_bytes = [0; 512];
        let mut vec_buf;

        let buf = if let Some(buf) = fixed_bytes.get_mut(0 .. Self::LEN) {
            buf
        } else {
            vec_buf = vec![0; Self::LEN];
            &mut vec_buf[..]
        };

        let rest = self.encode_verbatim(&mut buf[..]).unwrap();
        assert_eq!(rest.len(), 0);

        Digest::hash_verbatim_bytes(buf)
    }
}
*/

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

#[cfg(test)]
mod tests {
}
