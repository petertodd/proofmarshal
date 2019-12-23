use super::*;

impl<Z, T: Encoded<Z>, const N: usize> Encoded<Z> for [T; N] {
    type Encoded = [T::Encoded; N];
}

impl<'a, Z: Zone, T: Encode<'a, Z>, const N: usize> Encode<'a, Z> for [T; N] {
    type State = [T::State; N];

    fn save_children(&'a self) -> Self::State {
        todo!()
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        for (item, state) in self.iter().zip(state.iter_mut()) {
            dumper = item.poll(state, dumper)?;
        }
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &[T::State; N], dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}
