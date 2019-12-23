use super::*;

use nonzero::NonZero;

impl<Z, T: Encoded<Z>> Encoded<Z> for Option<T> {
    type Encoded = Option<T::Encoded>;
}

impl<'a, Z: Zone, T: NonZero + Encode<'a, Z>> Encode<'a, Z> for Option<T> {
    type State = Option<T::State>;

    fn save_children(&'a self) -> Self::State {
        self.as_ref().map(T::save_children)
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        match (self, state) {
            (Some(value), Some(state)) => value.poll(state, dumper),
            (None, None) => Ok(dumper),
            _ => unreachable!("invalid state"),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Option<T::State>, dst: W) -> Result<W::Ok, W::Error> {
        // FIXME: assertions?
        match (self, state) {
            (None, None) => {
                dst.write_padding(mem::size_of::<T::Encoded>())?
                    .finish()
            },
            (Some(value), Some(state)) => value.encode_blob(state, dst),
            _ => unreachable!("invalid state"),
        }
    }
}
