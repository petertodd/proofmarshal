//! Verbatim encoding.

#![feature(never_type)]

use std::io;

pub mod primitive;
pub mod option;
pub mod array;

pub trait Verbatim<P = !> : Sized {
    type Error;

    /// The length of the verbatim encoding.
    const LEN: usize;

    /// Whether part of this encoding contains bytes that always have at least one non-zero byte.
    ///
    /// If `NONZERO_NICHE == true` containers like `Option<T>` can use an *all* zero encoding as
    /// the absense of value.
    const NONZERO_NICHE: bool;

    fn encode<W: io::Write>(&self, dst: W, ptr_encoder: &mut impl PtrEncode<P>) -> Result<W, io::Error>;
    fn decode(src: &[u8], ptr_decoder: &mut impl PtrDecode<P>) -> Result<Self, Self::Error>;
}

pub unsafe trait PtrDecode<P> {
}

pub unsafe trait PtrEncode<P> {
}

/// It's safe for anything to encode pointers that don't exist.
unsafe impl<T: ?Sized> PtrEncode<!> for T {}

/// It's safe for anything to decode pointers that don't exist.
unsafe impl<T: ?Sized> PtrDecode<!> for T {}

unsafe impl PtrEncode<()> for () {}
unsafe impl PtrDecode<()> for () {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit() {
        let encoded = Verbatim::<!>::encode(&(), Vec::<u8>::new(), &mut ()).unwrap();
        assert_eq!(encoded, &[]);
    }

    #[test]
    fn primitives() {
        let encoded = Verbatim::<!>::encode(&42u8, Vec::<u8>::new(), &mut ()).unwrap();
        assert_eq!(encoded, &[42]);
    }
}
