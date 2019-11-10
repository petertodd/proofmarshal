use super::*;

#[derive(Debug)]
pub struct DecodeTupleError;

impl<Z, A: Encode<Z>, B: Encode<Z>> Encode<Z> for (A,B) {
    const ENCODING: Encoding = A::ENCODING.extend(B::ENCODING);

    fn encode<E: Encoder<Zone=Z>>(&self, encoder: E) -> Result<E::Done, E::Error> {
        encoder.encode_tuple()?
            .encode_elem(&self.0)?
            .encode_elem(&self.1)?
            .end()
    }
}

impl<Z, A: Decode<Z>, B: Decode<Z>> Decode<Z> for (A,B) {
    type Error = DecodeTupleError;

    fn decode<D: Decoder<Zone=Z>>(_decoder: D) -> Result<(D::Done, Self), Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!((42u8, true).encode(vec![]).unwrap(),
                   &[42,1]);
    }
}
