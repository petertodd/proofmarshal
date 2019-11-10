use std::marker::PhantomData;

use crate::*;
use crate::marshal::{*, de::*, en::*};

pub struct Pile<'p> {
    buf: &'p [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    raw: u64,
}

#[derive(Debug)]
pub struct LoadOffsetError;

impl<'p> Zone for Pile<'p> {
    type Error = LoadOffsetError;

    type Ptr = Offset<'p>;
    type PersistPtr = u64;
}

impl<'p> Encoding<Self> for Pile<'p> {
    const PILE_ENCODING: PileEncoding = PileEncoding::new(0);
}

impl<'p> Encode<Self> for Pile<'p> {
    fn encode<E: Encoder<Zone=Self>>(&self, encoder: E) -> Result<E::Done, E::Error> {
        todo!()
    }
}

impl<'p> Decode<Self> for Pile<'p> {
    type Error = !;
    fn decode<D: Decoder<Zone=Self>>(decoder: D) -> Result<(D::Done, Self), Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
