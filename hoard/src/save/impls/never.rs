use super::*;

impl<R> Encoded<R> for ! {
    type Encoded = Self;
}

impl<Q, R> Encode<'_, Q, R> for ! {
    type State = !;
    fn init_encode_state(&self) -> ! { *self }

    fn encode_poll<D>(&self, _: &mut !, _: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        match *self {}
    }

    fn encode_blob<W: WriteBlob>(&self, _: &!, _: W) -> Result<W::Done, W::Error> {
        match *self {}
    }

}
