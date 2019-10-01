//! Verbatim serialization/deserialization

use core::borrow::Borrow;

use crate::ptr::{Ptr, Dealloc};
use crate::types::Type;
use crate::arena::Arena;

pub use crate::util::fixedbytes::{ReadBytes, WriteBytes};

pub mod types;
pub mod layout;
use self::layout::Layout;

mod boilerplate;

/// Types whose values can be verbatim encoded.
pub trait Verbatim : Sized + Type {
    type Layout : Layout + Default;
}

/// The ability to encode verbatim.
pub trait Encode<P> : Verbatim {
    fn encode<E: Encoder>(&self, encoder: E) -> Result<E::Ok, E::Error>
        where P: Borrow<E::Ptr>;
}

/// A encoder that `Verbatim` types can encode too.
pub trait Encoder {
    type Ptr : Verbatim + Dealloc;

    /// Returned if the encoder has an error.
    type Error;

    /// The output type produced by the `Encoder` for a succesful encoding.
    type Ok;

    /// Write raw bytes to the encoder.
    fn write_bytes(&mut self, buf: impl AsRef<[u8]>) -> Result<(), Self::Error>;

    /// Write zeros.
    #[inline]
    fn write_zeros(&mut self, len: usize) -> Result<(), Self::Error> {
        for _ in 0 .. len {
            self.write_bytes([0])?;
        }
        Ok(())
    }

    /// Write a `Encode` capable value.
    fn write_value<T: Encode<Self::Ptr>>(&mut self, value: &T) -> Result<(), Self::Error>;

    fn write_ptr<T: ?Sized + Type>(&mut self, ptr: &Ptr<T,Self::Ptr>) -> Result<(), Self::Error>;

    /// Finish the encoding of this value.
    fn done(self) -> Result<Self::Ok, Self::Error>;
}

/*
impl Encoder<Missing> for Vec<u8> {
    type Error = !;
    type Done = Self;

    fn write_bytes(&mut self, buf: impl AsRef<[u8]>) -> Result<(), Self::Error> {
        self.extend_from_slice(buf.as_ref());
        Ok(())
    }

    fn write<T: Verbatim<Missing>>(&mut self, value: &T) -> Result<(), Self::Error> {
        let mut b = vec![];
        let b = value.encode(b)?;
        dbg!(&b);
        self.write_bytes(b)?;
        Ok(())
    }

    fn write_ptr<T: ?Sized + Type>(&mut self, ptr: &Ptr<T,Missing>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn done(self) -> Result<Self, !> {
        Ok(self)
    }
}
*/

/*pub trait Encode<P> : Sized + Type {
    //type DecodeError;
    //fn decode<D: Decoder<Arena=A>>(decoder: D) -> Result<(Self, D::Done), Self::DecodeError>;
}*/

/*
pub trait Decoder {
    type Arena : ?Sized + VerbatimArena;
    type Done;

    fn read_bytes<B: AsMut<[u8]>>(&mut self, buf: B) -> B;
    fn read_value<T: Verbatim<Self::Arena>>(&mut self) -> Result<T, T::DecodeError>;

    fn read_ptr<T: ?Sized + Type>(&mut self, encoded_ptr: <Self::Arena as VerbatimArena>::EncodedPtr)
        -> Result<Ptr<T, Self::Arena>, <Self::Arena as VerbatimArena>::PtrError>;

    fn done(self) -> Self::Done;
}

impl<A: ?Sized + VerbatimArena> Verbatim<A> for ! {
    fn is_nonzero() -> bool {
        true
    }

    fn len() -> usize {
        usize::max_value()
    }

    fn encode<E: Encoder<A>>(&self, _encoder: E) -> Result<E::Done, E::Error> {
        match *self {}
    }

    type DecodeError = !;
    fn decode<D: Decoder<Arena=A>>(_decoder: D) -> Result<(Self, D::Done), Self::DecodeError> {
        panic!("attempt to decode !")
    }
}

impl VerbatimArena for ! {
    type EncodedPtr = !;
    type PtrError = !;
}


impl<T: ?Sized + Type, A: ?Sized + VerbatimArena> Verbatim<A> for Ptr<T,A>
where A::Ptr: Verbatim<A>,
      T::Metadata: Verbatim<A>,
{
    fn is_nonzero() -> bool {
        A::Ptr::is_nonzero() && T::Metadata::is_nonzero()
    }

    fn len() -> usize {
        A::Ptr::len() + T::Metadata::len()
    }

    fn encode<E: Encoder<A>>(&self, mut encoder: E) -> Result<E::Done, E::Error> {
        let encoded_ptr = encoder.write_ptr(self)?;
        encoder.write(&encoded_ptr)?;
        encoder.done()
    }

    type DecodeError = !;
    fn decode<D: Decoder<Arena=A>>(mut decoder: D) -> Result<(Self, D::Done), Self::DecodeError> {
        let encoded_ptr: A::EncodedPtr = decoder.read_value().ok().unwrap();

        let this = decoder.read_ptr(encoded_ptr).ok().unwrap();

        Ok((this, decoder.done()))
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let b = Verbatim::<Missing>::encode(&Some(0xaabb_u16), vec![]).unwrap();
        dbg!(b);

        let v: Option<u64> = None;
        let b = Verbatim::<Missing>::encode(&v, vec![]).unwrap();
        dbg!(b);
    }
}*/
