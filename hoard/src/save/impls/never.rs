use super::*;

impl<Q, R> Encode<Q, R> for ! {
    type EncodePoll = Self;

    fn init_encode(&self, dst: &impl SavePtr) -> Self::EncodePoll {
        match *self {}
    }
}

impl<Q, R> SavePoll<Q, R> for ! {
    fn save_poll<D: SavePtr>(&mut self, dst: D) -> Result<D, D::Error> {
        match *self {}
    }
}

impl EncodeBlob for ! {
    const BLOB_LEN: usize = 0;
    fn encode_blob<W: WriteBlob>(&self, _: W) -> Result<W::Done, W::Error> {
        match *self {}
    }
}
