use std::convert::TryFrom;
use std::ops::Range;
use std::num::NonZeroU8;
use std::fmt;

use thiserror::Error;


/// The height of a perfect binary tree.
///
/// Valid range: `0 ..= 63`
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(u8);

/// The height of an inner node in a perfect binary tree.
///
/// Valid range: `1 ..= 63`
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroHeight(NonZeroU8);

/// Unsized height.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynHeight([()]);

/// Unsized non-zero height.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynNonZeroHeight([()]);

pub trait ToHeight {
    fn to_height(&self) -> Height;
}

pub trait ToNonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight;
}

impl<T: ?Sized + ToNonZeroHeight> ToHeight for T {
    fn to_height(&self) -> Height {
        self.to_nonzero_height().into()
    }
}


// ----- conversions ------

impl From<NonZeroHeight> for Height {
    fn from(height: NonZeroHeight) -> Height {
        Self(height.0.get())
    }
}

impl From<Height> for u8 {
    fn from(height: Height) -> u8 {
        height.0
    }
}

impl From<NonZeroHeight> for NonZeroU8 {
    fn from(height: NonZeroHeight) -> Self {
        height.0
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct HeightError;

impl TryFrom<Height> for NonZeroHeight {
    type Error = HeightError;
    fn try_from(n: Height) -> Result<Self, Self::Error> {
        n.assert_valid();
        match n.0 {
            0 => Err(HeightError),
            n => Ok(unsafe { NonZeroHeight::new_unchecked(NonZeroU8::new_unchecked(n)) }),
        }
    }
}

impl TryFrom<usize> for Height {
    type Error = HeightError;
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        u8::try_from(n).ok()
           .and_then(Height::new)
           .ok_or(HeightError)
    }
}

impl TryFrom<usize> for NonZeroHeight {
    type Error = HeightError;
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        u8::try_from(n).ok()
           .and_then(NonZeroU8::new)
           .and_then(NonZeroHeight::new)
           .ok_or(HeightError)
    }
}

impl Height {
    pub const MAX: u8 = 63;

    #[inline(always)]
    fn assert_valid(&self) {
        assert!(self.0 <= Self::MAX);
    }

    #[inline(always)]
    pub fn new(n: u8) -> Option<Self> {
        if n <= Self::MAX {
            Some(Self(n))
        } else {
            None
        }
    }

    pub const unsafe fn new_unchecked(n: u8) -> Self {
        Self(n)
    }

    #[inline(always)]
    pub fn get(self) -> u8 {
        self.0
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

impl NonZeroHeight {
    #[inline(always)]
    fn assert_valid(&self) {
        debug_assert!(self.0.get() <= Height::MAX);
    }

    #[inline(always)]
    pub fn new(n: NonZeroU8) -> Option<Self> {
        if n.get() <= Height::MAX {
            Some(Self(n))
        } else {
            None
        }
    }

    pub const unsafe fn new_unchecked(n: NonZeroU8) -> Self {
        Self(n)
    }

    pub fn decrement(self) -> Height {
        Height::new(self.0.get() - 1).unwrap()
    }
}

impl ToHeight for Height {
    fn to_height(&self) -> Height {
        *self
    }
}

impl ToNonZeroHeight for NonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight {
        *self
    }
}

impl ToHeight for DynHeight {
    fn to_height(&self) -> Height {
        let n = self.0.len();
        debug_assert!(n < Height::MAX as usize);
        unsafe { Height::new_unchecked(n as u8) }
    }
}

impl ToNonZeroHeight for DynNonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight {
        let n = self.0.len();
        debug_assert!(n < Height::MAX as usize);
        debug_assert!(n != 0);
        unsafe {
            NonZeroHeight::new_unchecked(NonZeroU8::new_unchecked(n as u8))
        }
    }
}

// fmt::Debug impls
impl fmt::Debug for DynHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("DynHeight")
            .field(&self.0.len())
            .finish()
    }
}

impl fmt::Debug for DynNonZeroHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("DynNonZeroHeight")
            .field(&self.0.len())
            .finish()
    }
}


#[derive(Debug)]
pub(super) struct DummyHeight;

impl ToHeight for DummyHeight {
    fn to_height(&self) -> Height {
        panic!()
    }
}

use hoard::primitive::Primitive;
use hoard::blob::{Bytes, BytesUninit};

impl Primitive for Height {
    const BLOB_SIZE: usize = 1;

    type DecodeBytesError = HeightError;

    fn encode_blob_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        todo!()
    }

    fn decode_blob_bytes(_: Bytes<'_, Self>) -> Result<Self, HeightError> {
        todo!()
    }
}

impl Primitive for NonZeroHeight {
    const BLOB_SIZE: usize = 1;
    type DecodeBytesError = HeightError;

    fn encode_blob_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        todo!()
    }

    fn decode_blob_bytes(_: Bytes<'_, Self>) -> Result<Self, HeightError> {
        todo!()
    }
}
