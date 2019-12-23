use super::*;

impl<Z, T: Encoded<Z>> Saved<Z> for [T] {
    type Saved = [T::Encoded];
}

impl<'a, Z: Zone, T: Encode<'a, Z>> Save<'a, Z> for [T] {
    /// Remember that `Vec<T::State>` doesn't actually allocate if `T::State` is a zero-sized-type.
    type State = Vec<T::State>;

    fn save_children(&'a self) -> Self::State {
        self.iter().map(T::save_children).collect()
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        for (item, state) in self.iter().zip(state.iter_mut()) {
            dumper = item.poll(state, dumper)?;
        }
        Ok(dumper)
    }

    fn save_blob<D: Dumper<Z>>(&self, state: &Self::State, dumper: D) -> Result<(D, D::PersistPtr), D::Error> {
        for (item, state) in self.iter().zip(state) {
            todo!()
        }
        todo!()
    }
}
