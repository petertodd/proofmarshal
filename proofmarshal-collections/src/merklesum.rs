use thiserror::Error;

pub trait MerkleSum<T: ?Sized> : 'static + Copy {
    const MAX: Self;
    type Error : std::error::Error;

    fn from_item(item: &T) -> Self;

    fn try_sum(left: &Self, right: &Self) -> Result<Self, Self::Error>;

    fn saturating_sum(left: &Self, right: &Self) -> Self {
        Self::try_sum(left, right).unwrap_or(Self::MAX)
    }
}

impl<T: ?Sized> MerkleSum<T> for () {
    const MAX: Self = ();
    type Error = !;

    fn from_item(_: &T) -> Self {}

    fn try_sum(_: &Self, _: &Self) -> Result<Self, Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("overflow")]
#[non_exhaustive]
pub struct OverflowError;

impl MerkleSum<u8> for u8 {
    const MAX: Self = u8::MAX;
    type Error = OverflowError;

    fn from_item(x: &u8) -> Self {
        *x
    }

    fn try_sum(lhs: &Self, rhs: &Self) -> Result<Self, Self::Error> {
        lhs.checked_add(*rhs)
           .ok_or(OverflowError)
    }
}
