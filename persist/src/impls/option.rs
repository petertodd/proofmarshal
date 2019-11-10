use super::*;

impl<Z: Zone, T: Encode<Z>> Encode<Z> for Option<T> {
    const SIZE: usize = T::SIZE + 1;

    type Encode = Option<T::Encode>;

    fn encode(self) -> Self::Encode {
        self.map(|value| value.encode())
    }
}

impl<Z: Zone, E: EncodePoll<Z>> EncodePoll<Z> for Option<E> {
    type Target = Option<E::Target>;

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: Saver<Zone = Z>
    {
        match self {
            None => Poll::Ready(Ok(())),
            Some(inner) => inner.poll(saver),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}
