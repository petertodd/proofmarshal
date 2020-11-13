use super::*;

impl<T: Commit> Commit for Option<T> {
    type Commitment = Option<T::Commitment>;

    fn to_commitment(&self) -> Self::Commitment {
        self.as_ref().map(T::to_commitment)
    }
}
