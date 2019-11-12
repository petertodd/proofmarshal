use super::*;

#[derive(Debug)]
pub struct TupleError;

impl<Z: Zone, A: Save<Z>, B: Save<Z>> Save<Z> for (A,B) {
    const BLOB_LAYOUT: BlobLayout = A::BLOB_LAYOUT.extend(B::BLOB_LAYOUT);

    type SavePoll = (A::SavePoll, B::SavePoll);
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        let (a,b) = this.take_sized();
        (A::save_poll(a), B::save_poll(b))
    }
}

impl<Z: Zone, A: SavePoll<Z>, B: SavePoll<Z>> SavePoll<Z> for (A,B)
where A::Target: Sized, B::Target: Sized,
{
    type Target = (A::Target, B::Target);

    fn save_children<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Z>
    {
        let mut all_done = true;

        all_done &= self.0.save_children(saver)?.is_ready();
        all_done &= self.1.save_children(saver)?.is_ready();

        match all_done {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write(&self.0)?
           .write(&self.1)?
           .done()
    }
}

impl<Z: Zone, A: Load<Z>, B: Load<Z>> Load<Z> for (A,B) {
    type Error = TupleError;

    type ValidateChildren = (A::ValidateChildren, B::ValidateChildren);

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        let mut blob = blob.validate();
        let state = (
            blob.field::<A>().map_err(|_| TupleError)?,
            blob.field::<B>().map_err(|_| TupleError)?,
        );
        Ok(blob.done(state))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self {
        let mut f = blob.decode_struct(loader);
        (
            f.field::<A>(),
            f.field::<B>(),
        )
    }
}

impl<Z: Zone, A: ValidateChildren<Z>, B: ValidateChildren<Z>> ValidateChildren<Z> for (A,B) {
    fn validate_children<V>(&mut self, v: &mut V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        let mut all_done = true;

        all_done &= self.0.validate_children(v)?.is_ready();
        all_done &= self.1.validate_children(v)?.is_ready();

        match all_done {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::convert::TryFrom;

    #[test]
    fn test() {
        assert_eq!(encode((1u8,Some(2u8))),
                   &[1,1,2]);

        let blob = Blob::<(u8, Option<u8>), !>::try_from(&[1,1,2][..]).unwrap();
        assert_eq!(try_decode(blob).unwrap(),
                   Ref::Owned((1,Some(2))));
    }
}
