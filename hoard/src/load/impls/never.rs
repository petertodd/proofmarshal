use super::*;

impl<Z> Decode<Z> for ! {
    fn decode_blob(_: ValidBlob<Self>, _: &Z) -> Self {
        panic!()
    }
}
