use super::*;

use thiserror::Error;

impl<R, T: Encoded<R>> Encoded<R> for Option<T> {
    type Encoded = Option<T::Encoded>;
}

impl<'a, Q, R, T: Encode<'a, Q, R>> Encode<'a, Q, R> for Option<T> {
    type State = Option<T::State>;

    fn init_encode_state(&'a self) -> Self::State {
        self.as_ref().map(T::init_encode_state)
    }

    fn encode_poll<D>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        match (self, state) {
            (Some(value), Some(state)) => value.encode_poll(state, dst),
            (None, None) => Ok(dst),
            _ => panic!(),
        }
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob
    {
        match (self, state) {
            (Some(value), Some(state)) => {
                dst.write_bytes(&[1])?
                   .write(value, state)?
                   .done()
            },
            (None, None) => {
                dst.write_bytes(&[0])?
                   .write_padding(<T::Encoded as ValidateBlob>::BLOB_LEN)?
                   .done()
            },
            _ => panic!(),
        }
    }
}
