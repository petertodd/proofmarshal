use std::convert::TryFrom;
use std::num::NonZeroU8;

use thiserror::Error;

use proofmarshal_derive::{Commit, Prune};

/// The height of a perfect binary tree.
///
/// Valid range: `0 ..= 63`
#[derive(Commit, Prune, Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(u8);

impl Height {
    pub const MAX: u8 = 63;

    #[inline(always)]
    fn assert_valid(&self) {
        assert!(self.0 <= Self::MAX);
    }

    #[inline(always)]
    pub fn new(n: u8) -> Result<Self, TryFromIntError> {
        if n <= Self::MAX {
            Ok(Self(n))
        } else {
            Err(TryFromIntError)
        }
    }

    pub const unsafe fn new_unchecked(n: u8) -> Self {
        Self(n)
    }

    #[inline(always)]
    pub fn len(self) -> usize {
        self.assert_valid();
        1 << self.0
    }

    #[inline]
    pub fn try_increment(self) -> Option<NonZeroHeight> {
        if self.0 < Self::MAX {
            Some(NonZeroHeight::new(NonZeroU8::new(self.0 + 1).unwrap()).unwrap())
        } else {
            assert!(self.0 == Self::MAX);
            None
        }
    }
}


/// The height of an inner node in a perfect binary tree.
///
/// Valid range: `1 ..= 63`
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroHeight(NonZeroU8);

impl NonZeroHeight {
    #[inline(always)]
    pub fn new(n: NonZeroU8) -> Result<Self, TryFromIntError> {
        if n.get() <= Height::MAX {
            Ok(Self(n))
        } else {
            Err(TryFromIntError)
        }
    }

    pub const unsafe fn new_unchecked(n: NonZeroU8) -> Self {
        Self(n)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct TryFromIntError;

impl TryFrom<u8> for Height {
    type Error = TryFromIntError;
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        Self::new(n)
    }
}

impl TryFrom<NonZeroU8> for Height {
    type Error = TryFromIntError;
    fn try_from(n: NonZeroU8) -> Result<Self, Self::Error> {
        Self::new(n.get())
    }
}

impl From<Height> for u8 {
    fn from(height: Height) -> u8 {
        height.0
    }
}

impl From<NonZeroHeight> for Height {
    fn from(height: NonZeroHeight) -> Height {
        Self(height.0.get())
    }
}
