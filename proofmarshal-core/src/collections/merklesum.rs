use thiserror::Error;

use hoard::blob::Blob;

use crate::commit::Commit;

pub trait MerkleSum<T: ?Sized> : 'static + Copy + Blob + Commit<Committed=Self> {
    fn from_item(item: &T) -> Self;

    fn sum(left: Self, right: Self) -> Self;
}

impl<T: ?Sized> MerkleSum<T> for () {
    fn from_item(_: &T) -> Self {}

    fn sum(_: Self, _: Self) -> Self {
        ()
    }
}

pub trait TryMerkleSum<T: ?Sized> : 'static + Copy + Blob + Commit<Committed=Self> {
    type Error : 'static + std::error::Error;

    fn from_item(item: &T) -> Self;

    fn try_sum(left: Self, right: Self) -> Result<Self, Self::Error>;
}

impl<T: ?Sized, S: MerkleSum<T>> TryMerkleSum<T> for S {
    type Error = !;

    fn from_item(item: &T) -> Self {
        S::from_item(item)
    }

    fn try_sum(left: Self, right: Self) -> Result<Self, Self::Error> {
        Ok(S::sum(left, right))
    }
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("FIXME")]
#[non_exhaustive]
pub struct OverflowError;

impl TryMerkleSum<Self> for u8 {
    type Error = OverflowError;

    fn from_item(n: &Self) -> Self {
        *n
    }

    fn try_sum(left: Self, right: Self) -> Result<Self, Self::Error> {
        left.checked_add(right).ok_or(OverflowError)
    }
}

impl<T: ?Sized, S: TryMerkleSum<T>> MerkleSum<T> for Option<S> {
    fn from_item(item: &T) -> Self {
        Some(S::from_item(item))
    }

    /// Sums the values if both are available.
    ///
    /// ```
    /// # use proofmarshal_core::collections::merklesum::MerkleSum;
    /// assert_eq!(<Option<u8> as MerkleSum<u8>>::sum(Some(1), Some(2)),
    ///            Some(3));
    /// ```
    ///
    /// ```
    /// # use proofmarshal_core::collections::merklesum::MerkleSum;
    /// assert_eq!(<Option<u8> as MerkleSum<u8>>::sum(Some(255), Some(1)),
    ///            None);
    /// assert_eq!(<Option<u8> as MerkleSum<u8>>::sum(Some(1), None),
    ///            None);
    /// assert_eq!(<Option<u8> as MerkleSum<u8>>::sum(None, Some(2)),
    ///            None);
    /// assert_eq!(<Option<u8> as MerkleSum<u8>>::sum(None, None),
    ///            None);
    /// ```
    fn sum(left: Self, right: Self) -> Self {
        if let (Some(left), Some(right)) = (left, right) {
            S::try_sum(left, right).ok()
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8_sum_overflow() {
        assert_eq!(<u8 as TryMerkleSum<u8>>::try_sum(1,2), Ok(3));
        assert_eq!(<u8 as TryMerkleSum<u8>>::try_sum(255,0), Ok(255));
        assert_eq!(<u8 as TryMerkleSum<u8>>::try_sum(255,1), Err(OverflowError));
    }
}
