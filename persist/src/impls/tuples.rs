use super::*;

use core::marker::PhantomData;

#[derive(Debug)]
pub struct TupleEncoder<E, Z> {
    marker: PhantomData<fn(Z) -> Z>,
    inner: E,
}

impl<E, Z> From<E> for TupleEncoder<E, Z> {
    fn from(inner: E) -> Self {
        TupleEncoder { marker: PhantomData, inner }
    }
}

impl<Z: Zone, A: Encode<Z>, B: Encode<Z>> Encode<Z> for (A,B) {
    const BLOB_LAYOUT: BlobLayout = A::BLOB_LAYOUT.extend(B::BLOB_LAYOUT);
    type Encode = TupleEncoder<(A::Encode, B::Encode), Z>;

    fn encode(self) -> Self::Encode {
        (self.0.encode(), self.1.encode()).into()
    }
}

impl<Z: Zone, A: EncodePoll<Zone=Z>, B: EncodePoll<Zone=Z>> EncodePoll for TupleEncoder<(A,B), Z>
where A::Target: Encode<Z>,
      B::Target: Encode<Z>
{
    type Zone = Z;
    type Target = (A::Target, B::Target);

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: Saver<Zone = Self::Zone>
    {
        let mut all_done = true;

        all_done &= self.inner.0.poll(saver)?.is_ready();
        all_done &= self.inner.1.poll(saver)?.is_ready();

        match all_done {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write(&self.inner.0)?
           .write(&self.inner.1)?
           .done()
    }
}
