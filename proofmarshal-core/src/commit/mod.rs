//! Cryptographic commitments.

use std::mem;

mod impls;

pub mod digest;
pub use self::digest::Digest;

/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit {
    const VERBATIM_LEN: usize;
    type Committed : 'static;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim);

    fn to_verbatim(&self) -> Vec<u8> {
        let mut r = vec![];
        self.encode_verbatim(&mut r);
        r
    }

    fn commit(&self) -> Digest<Self::Committed> {
        if Self::VERBATIM_LEN > 32 {
            let mut hasher = CommitHasher::new();
            self.encode_verbatim(&mut hasher);
            hasher.finalize().into()
        } else {
            let mut buf = [0u8; 32];
            self.encode_verbatim(&mut &mut buf[..]);
            buf.into()
        }
    }
}

impl<T: ?Sized + Commit> Commit for &'_ T {
    const VERBATIM_LEN: usize = T::VERBATIM_LEN;
    type Committed = T::Committed;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        (&**self).encode_verbatim(dst)
    }

    fn commit(&self) -> Digest<Self::Committed> {
        (&**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for &'_ mut T {
    const VERBATIM_LEN: usize = T::VERBATIM_LEN;
    type Committed = T::Committed;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        (&**self).encode_verbatim(dst)
    }

    fn commit(&self) -> Digest<Self::Committed> {
        (&**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for Box<T> {
    const VERBATIM_LEN: usize = T::VERBATIM_LEN;
    type Committed = T::Committed;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        (&**self).encode_verbatim(dst)
    }

    fn commit(&self) -> Digest<Self::Committed> {
        (&**self).commit()
    }
}

pub trait WriteVerbatim {
    fn write_bytes(&mut self, src: &[u8]);

    fn write_zeros(&mut self, len: usize) {
        for _ in 0 .. len {
            self.write_bytes(&[0]);
        }
    }

    fn write<T: ?Sized + Commit>(&mut self, src: &T) {
        struct Limit<W> {
            inner: W,
            remaining: usize,
        }

        impl<W: WriteVerbatim> WriteVerbatim for Limit<W> {
            #[inline]
            fn write_bytes(&mut self, src: &[u8]) {
                self.remaining = self.remaining.checked_sub(src.len())
                                               .expect("overflow");
                self.inner.write_bytes(src);
            }
        }

        let mut limited = Limit {
            inner: self,
            remaining: T::VERBATIM_LEN,
        };

        src.encode_verbatim(&mut limited);
        assert_eq!(limited.remaining, 0, "not all bytes written");
    }
}

impl WriteVerbatim for Vec<u8> {
    #[inline]
    fn write_bytes(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }

    fn write<T: ?Sized + Commit>(&mut self, src: &T) {
        let start = self.len();
        src.encode_verbatim(self);
        assert_eq!(self.len() - start, T::VERBATIM_LEN,
                   "not all bytes written");
    }
}

impl WriteVerbatim for &'_ mut [u8] {
    fn write_bytes(&mut self, src: &[u8]) {
        assert!(self.len() >= src.len(), "overflow");

        let this = mem::take(self);
        let (dst, rest) = this.split_at_mut(src.len());
        dst.copy_from_slice(src);
        *self = rest;
    }
}

impl<T: ?Sized + WriteVerbatim> WriteVerbatim for &'_ mut T {
    fn write_bytes(&mut self, src: &[u8]) {
        (**self).write_bytes(src)
    }
}

use sha2::Digest as _;

#[derive(Debug)]
pub struct CommitHasher {
    inner: sha2::Sha256,
}

impl CommitHasher {
    pub fn new() -> Self {
        Self {
            inner: sha2::Sha256::new(),
        }
    }

    pub fn finalize(self) -> [u8; 32] {
        let d = self.inner.result();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&d[..]);
        digest
    }
}

impl WriteVerbatim for CommitHasher {
    #[inline]
    fn write_bytes(&mut self, src: &[u8]) {
        self.inner.input(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
