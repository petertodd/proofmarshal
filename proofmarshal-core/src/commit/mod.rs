//! Cryptographic commitments.

/*
use hoard::pointee::Pointee;

pub mod digest;
pub use self::digest::Digest;

*/
mod impls;

pub trait Verbatim {
    const VERBATIM_LEN: usize;
    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim);

    fn encode_verbatim(&self) -> Vec<u8> {
        let mut r = vec![];
        r.write(self);
        r
    }
}

pub trait WriteVerbatim {
    fn write_bytes(&mut self, src: &[u8]);

    fn write_zeros(&mut self, len: usize) {
        for _ in 0 .. len {
            self.write_bytes(&[0]);
        }
    }

    fn write<T: ?Sized + Verbatim>(&mut self, src: &T) {
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

        src.encode_verbatim_in(&mut limited);
        assert_eq!(limited.remaining, 0, "not all bytes written");
    }
}

impl WriteVerbatim for Vec<u8> {
    #[inline]
    fn write_bytes(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }

    fn write<T: ?Sized + Verbatim>(&mut self, src: &T) {
        let start = self.len();
        src.encode_verbatim_in(self);
        assert_eq!(self.len() - start, T::VERBATIM_LEN,
                   "not all bytes written");
    }
}

impl<T: ?Sized + WriteVerbatim> WriteVerbatim for &'_ mut T {
    fn write_bytes(&mut self, src: &[u8]) {
        (**self).write_bytes(src)
    }
}

/*
impl<T: ?Sized + Verbatim> Verbatim for &'_ T {
    const LEN: usize = T::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}

impl<T: ?Sized + Verbatim> Verbatim for &'_ mut T {
    const LEN: usize = T::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}

impl<T: ?Sized + Verbatim> Verbatim for Box<T> {
    const LEN: usize = T::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}
*/

/*
/// The ability to cryptographically commit to a value of this type.
///
/// Usually, but not always, this means hashing the value in a deterministic way.
pub trait Commit : Verbatim {
    type Committed;

    fn commit(&self) -> Digest<Self::Committed> {
        Digest::hash_verbatim(self).cast()
    }
}

impl<T: ?Sized + Commit> Commit for &'_ T {
    type Committed = T::Committed;
}

impl<T: ?Sized + Commit> Commit for &'_ mut T {
    type Committed = T::Committed;
}

impl<T: ?Sized + Commit> Commit for Box<T> {
    type Committed = T::Committed;
}

pub trait WriteVerbatim {
    fn write_bytes(&mut self, src: &[u8]);

    fn write_zeros(&mut self, len: usize) {
        for _ in 0 .. len {
            self.write_bytes(&[0]);
        }
    }

    fn write<T: ?Sized + Verbatim>(&mut self, src: &T) {
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
            remaining: T::LEN,
        };

        src.encode_verbatim_in(&mut limited);
        assert_eq!(limited.remaining, 0, "not all bytes written");
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

    pub fn finalize(self) -> Digest {
        let d = self.inner.result();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&d[..]);
        digest.into()
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
*/
