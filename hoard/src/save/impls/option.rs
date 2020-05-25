use super::*;

use thiserror::Error;

#[derive(Debug)]
pub struct OptionEncoder<T>(Option<T>);

impl<Q, R, T: Encode<Q, R>> Encode<Q, R> for Option<T> {
    type EncodePoll = OptionEncoder<T::EncodePoll>;

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
        OptionEncoder(self.as_ref().map(|value| value.init_encode(dst)))
    }
}

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for OptionEncoder<T> {
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, dst: D) -> Result<D, D::Error> {
        match &mut self.0 {
            Some(inner) => inner.save_poll(dst),
            None => Ok(dst),
        }
    }
}

impl<T: EncodeBlob> EncodeBlob for OptionEncoder<T> {
    const BLOB_LEN: usize = 1 + T::BLOB_LEN;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        match &self.0 {
            Some(inner) => {
                dst.write_bytes(&[1])?
                   .write(inner)?
                   .done()
            },
            None => {
                dst.write_bytes(&[0])?
                   .write_padding(T::BLOB_LEN)?
                   .done()
            }
        }
    }
}
