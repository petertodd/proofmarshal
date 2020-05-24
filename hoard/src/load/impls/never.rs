use super::*;

impl<Z> Decode<Z> for ! {
    fn decode_blob(blob: BlobDecoder<Z, Self>) -> Self {
        panic!()
    }
}
