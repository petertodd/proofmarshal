use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::mem;
use std::ops;

use crate::Le;

pub type Word = Le<u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WordOffset(usize);

impl WordOffset {
    pub const WORD: Self = WordOffset(mem::size_of::<Word>());

    /// Creates a `WordOffset` by aligning an offset.
    pub fn align(offset: usize) -> Self {
        let size = mem::size_of::<Word>() as usize;
        let aligned = ((offset + size - 1) / size) * size;
        Self(aligned)
    }

    /// Returns the amount of padding necessary to align an offset to a word.
    pub fn align_padding(offset: usize) -> usize {
        let Self(aligned) = Self::align(offset);
        aligned - offset
    }

    pub fn get(self) -> usize {
        self.0
    }
}

impl AsRef<usize> for WordOffset {
    fn as_ref(&self) -> &usize {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnalignedError(pub usize);

/// Creates a `WordOffset` from an aligned `usize`.
impl TryFrom<usize> for WordOffset {
    type Error = UnalignedError;
    fn try_from(offset: usize) -> Result<Self, Self::Error> {
        let aligned = WordOffset::align(offset);

        if aligned == offset {
            Ok(aligned)
        } else {
            Err(UnalignedError(offset))
        }
    }
}

impl From<WordOffset> for usize {
    fn from(offset: WordOffset) -> usize {
        offset.0
    }
}

impl ops::AddAssign for WordOffset {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl ops::Add for WordOffset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        WordOffset(self.0 + rhs.0)
    }
}

impl ops::Sub for WordOffset {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        WordOffset(self.0 - rhs.0)
    }
}

impl ops::Sub<usize> for WordOffset {
    type Output = usize;
    fn sub(self, rhs: usize) -> usize {
        self.0 - rhs
    }
}

impl ops::Sub<WordOffset> for usize {
    type Output = usize;
    fn sub(self, rhs: WordOffset) -> usize {
        self - rhs.0
    }
}

impl cmp::PartialEq<usize> for WordOffset {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

impl cmp::PartialEq<WordOffset> for usize {
    fn eq(&self, other: &WordOffset) -> bool {
        *self == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_eq_usize() {
        assert_eq!(WordOffset(0), 0);
        assert_eq!(WordOffset(8), 8);
        assert_ne!(WordOffset(8), 7);
    }

    #[test]
    fn test_align() {
        assert_eq!(WordOffset::align(0), 0);
        assert_eq!(WordOffset::align(1), 8);
        assert_eq!(WordOffset::align(7), 8);
        assert_eq!(WordOffset::align(8), 8);
    }

    #[test]
    fn test_align_padding() {
        assert_eq!(WordOffset::align_padding(0), 0);
        assert_eq!(WordOffset::align_padding(1), 7);
        assert_eq!(WordOffset::align_padding(7), 1);
        assert_eq!(WordOffset::align_padding(8), 0);
    }

    #[test]
    fn test_try_from_usize() {
        assert_eq!(WordOffset::try_from(0), Ok(WordOffset(0)));
        assert_eq!(WordOffset::try_from(1), Err(UnalignedError(1)));
        assert_eq!(WordOffset::try_from(7), Err(UnalignedError(7)));
        assert_eq!(WordOffset::try_from(8), Ok(WordOffset(8)));
    }
}
