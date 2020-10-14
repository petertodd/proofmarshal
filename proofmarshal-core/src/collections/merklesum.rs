use crate::commit::Commit;
use hoard::primitive::Primitive;

pub trait MerkleSum<T: ?Sized> : 'static + Primitive + Commit<Committed=Self> {
    const MAX: Self;
    type Error : std::error::Error;

    fn from_item(item: &T) -> Self;

    fn try_sum(left: Self, right: Self) -> Result<Self, Self::Error>;

    fn saturating_sum(left: Self, right: Self) -> Self {
        Self::try_sum(left, right).unwrap_or(Self::MAX)
    }
}

impl<T: ?Sized> MerkleSum<T> for () {
    const MAX: Self = ();
    type Error = !;

    fn from_item(_: &T) -> Self {}

    fn try_sum(_: Self, _: Self) -> Result<Self, Self::Error> {
        Ok(())
    }
}
