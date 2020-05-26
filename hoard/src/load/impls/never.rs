use super::*;

impl<Q: Ptr> Decode<Q> for ! {
    fn decode_blob(blob: BlobDecoder<Q, Self>) -> Self {
        panic!()
    }
}
