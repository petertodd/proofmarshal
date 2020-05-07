use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::num::NonZeroU64;

use thiserror::Error;

use hoard::marshal::{Primitive, blob::*};
use hoard::pointee::{Metadata, MetadataKind};
use proofmarshal_derive::{Commit, Prune};

/// The length of a MMR.
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(packed)]
pub struct Length(u64);

/// The height of an inner node in a perfect binary tree.
///
/// Valid range: `1 ..= 63`
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(packed)]
pub struct NonZeroLength(NonZeroU64);

/// Dynamically sized length.
pub struct DynLength([()]);

impl fmt::Debug for DynLength {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0.len(), f)
    }
}

impl Length {
    pub const MAX: u64 = isize::max_value() as _;

    #[inline(always)]
    fn assert_valid(&self) {
        assert!(self.0 <= Self::MAX);
    }

    #[inline(always)]
    pub fn new(n: u64) -> Result<Self, TryFromIntError> {
        if n <= Self::MAX {
            Ok(Self(n))
        } else {
            Err(TryFromIntError)
        }
    }

    #[inline(always)]
    pub const unsafe fn new_unchecked(n: u64) -> Self {
        Self(n)
    }

    #[inline(always)]
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct TryFromIntError;

hoard::impl_encode_for_primitive!(Length, |this, dst| {
    dst.write_bytes(&this.0.to_le_bytes())?
        .finish()
});

hoard::impl_decode_for_primitive!(Length);

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct ValidateLengthError;

impl ValidateBlob for Length {
    type Error = ValidateLengthError;

    #[inline]
    fn validate<'a, V>(blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.validate_bytes(|blob| {
            let n = u64::from_le_bytes(blob[..].try_into().unwrap());
            if n <= Self::MAX {
                Ok(unsafe { blob.assume_valid() })
            } else {
                Err(ValidateLengthError)
            }
        })
    }
}

impl Primitive for Length {}

hoard::impl_encode_for_primitive!(NonZeroLength, |this, dst| {
    dst.write_bytes(&this.0.get().to_le_bytes())?
        .finish()
});

hoard::impl_decode_for_primitive!(NonZeroLength);

impl ValidateBlob for NonZeroLength {
    type Error = ValidateLengthError;

    #[inline]
    fn validate<'a, V>(blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.validate_bytes(|blob| {
            let n = u64::from_le_bytes(blob[..].try_into().unwrap());
            if 0 < n && n <= Length::MAX {
                Ok(unsafe { blob.assume_valid() })
            } else {
                Err(ValidateLengthError)
            }
        })
    }
}

impl Primitive for NonZeroLength {}

/*
impl Metadata for NonZeroHeight {
    #[inline]
    fn kind(&self) -> MetadataKind {
        MetadataKind::Len(self.0.get() as u64)
    }
}


impl NonZeroHeight {
    #[inline(always)]
    pub fn new(n: NonZeroU8) -> Result<Self, TryFromIntError> {
        if n.get() <= Height::MAX {
            Ok(Self(n))
        } else {
            Err(TryFromIntError)
        }
    }

    #[inline(always)]
    pub const unsafe fn new_unchecked(n: NonZeroU8) -> Self {
        Self(n)
    }

    #[inline]
    pub fn decrement(self) -> Height {
        Height::new(self.0.get().checked_sub(1).unwrap()).unwrap()
    }
}
*/

impl From<Length> for u64 {
    #[inline]
    fn from(n: Length) -> u64 {
        n.0
    }
}

impl From<NonZeroLength> for Length {
    #[inline]
    fn from(n: NonZeroLength) -> Length {
        Self(n.0.get())
    }
}

impl From<NonZeroLength> for u64 {
    #[inline]
    fn from(n: NonZeroLength) -> u64 {
        n.0.get()
    }
}

impl TryFrom<usize> for Length {
    type Error = TryFromIntError;

    fn try_from(n: usize) -> Result<Self, Self::Error> {
        Self::new(n as u64)
    }
}

pub unsafe trait GetLength {
    fn get(&self) -> Length;
}

unsafe impl GetLength for [()] {
    #[inline]
    fn get(&self) -> Length {
        Length::try_from(self.len()).expect("invalid height")
    }
}

unsafe impl GetLength for DynLength {
    #[inline]
    fn get(&self) -> Length {
        Length::try_from(self.0.len()).expect("invalid height")
    }
}

unsafe impl GetLength for Length {
    #[inline]
    fn get(&self) -> Length {
        *self
    }
}

unsafe impl GetLength for NonZeroLength {
    #[inline]
    fn get(&self) -> Length {
        Length::from(*self)
    }
}

unsafe impl GetLength for () {
    #[inline]
    fn get(&self) -> Length {
        panic!()
    }
}
