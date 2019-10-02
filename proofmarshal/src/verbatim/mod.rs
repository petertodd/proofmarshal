//! Verbatim encoding.

use std::io;

use crate::ptr::Ptr;

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

impl<P> Verbatim<P> for ! {
    type Error = !;
    const LEN: usize = 0;
    const NONZERO_NICHE: bool = false;

    fn encode<W: io::Write>(&self, _: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        match *self {}
    }
    fn decode(_: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        unreachable!("! can't be decoded")
    }
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

pub fn encode<T>(value: &T) -> Vec<u8>
where T: Verbatim<()>
{
    let buf = vec![];
    value.encode(buf, &mut ()).unwrap()
}

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
