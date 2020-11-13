//! Tree heights.

use std::convert::TryFrom;
use std::ops::Range;
use std::num::NonZeroU8;
use std::fmt;
use std::cmp;

use thiserror::Error;

use crate::impl_commit;
use crate::commit::Commit;
use crate::unreachable_unchecked;

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
pub struct HeightDyn([()]);

/// Unsized non-zero height.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroHeightDyn([()]);

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

impl_commit! {
    Height,
    NonZeroHeight
}

// ----- conversions ------

impl From<NonZeroHeight> for Height {
    #[inline(always)]
    fn from(height: NonZeroHeight) -> Height {
        Self(height.0.get())
    }
}

impl From<Height> for u8 {
    #[inline(always)]
    fn from(height: Height) -> u8 {
        height.0
    }
}

impl From<NonZeroHeight> for NonZeroU8 {
    #[inline(always)]
    fn from(height: NonZeroHeight) -> Self {
        height.0
    }
}

impl From<NonZeroHeight> for u8 {
    #[inline(always)]
    fn from(height: NonZeroHeight) -> Self {
        height.0.get()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct HeightError;

impl TryFrom<Height> for NonZeroHeight {
    type Error = HeightError;
    #[inline(always)]
    fn try_from(n: Height) -> Result<Self, Self::Error> {
        n.assert_valid();
        match n.0 {
            0 => Err(HeightError),
            n => Ok(unsafe { NonZeroHeight::new_unchecked(NonZeroU8::new_unchecked(n)) }),
        }
    }
}

impl TryFrom<u8> for Height {
    type Error = HeightError;
    #[inline(always)]
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        Self::new(n).ok_or(HeightError)
    }
}

impl TryFrom<usize> for Height {
    type Error = HeightError;
    #[inline(always)]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        u8::try_from(n).ok()
           .and_then(Height::new)
           .ok_or(HeightError)
    }
}

impl TryFrom<usize> for NonZeroHeight {
    type Error = HeightError;
    #[inline(always)]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        u8::try_from(n).ok()
           .and_then(NonZeroU8::new)
           .and_then(NonZeroHeight::new)
           .ok_or(HeightError)
    }
}

impl Height {
    pub const MAX: u8 = 63;
    pub const ZERO: Self = unsafe { Self::new_unchecked(0) };

    #[inline(always)]
    fn assert_valid(&self) {
        debug_assert!(self.0 <= Self::MAX);
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
    pub const fn get(self) -> u8 {
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
            let n = unsafe { NonZeroU8::new_unchecked(self.0 + 1) };
            let n = unsafe { NonZeroHeight::new_unchecked(n) };
            Some(n)
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

    #[inline(always)]
    pub fn get(self) -> NonZeroU8 {
        self.0
    }

    pub const unsafe fn new_unchecked(n: NonZeroU8) -> Self {
        Self(n)
    }

    pub fn decrement(self) -> Height {
        Height::new(self.0.get() - 1)
               .unwrap_or_else(|| unsafe { unreachable_unchecked!() })
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

impl ToHeight for HeightDyn {
    fn to_height(&self) -> Height {
        let n = self.0.len();
        debug_assert!(n < Height::MAX as usize);
        unsafe { Height::new_unchecked(n as u8) }
    }
}

impl ToNonZeroHeight for NonZeroHeightDyn {
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
impl fmt::Debug for HeightDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("HeightDyn")
            .field(&self.0.len())
            .finish()
    }
}

impl fmt::Debug for NonZeroHeightDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("NonZeroHeightDyn")
            .field(&self.0.len())
            .finish()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub struct DummyHeight;

impl ToHeight for DummyHeight {
    fn to_height(&self) -> Height {
        panic!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub struct DummyNonZeroHeight;

impl ToNonZeroHeight for DummyNonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight {
        panic!()
    }
}

use hoard::primitive::Primitive;
use hoard::blob::{Bytes, BytesUninit};

impl Primitive for Height {
    const BLOB_SIZE: usize = 1;

    type DecodeBytesError = HeightError;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.0)
           .done()
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, HeightError> {
        let mut fields = src.struct_fields();
        let height = fields.trust_field().into_ok();
        fields.assert_done();
        Self::new(height).ok_or(HeightError)
    }
}

impl Primitive for NonZeroHeight {
    const BLOB_SIZE: usize = 1;
    type DecodeBytesError = HeightError;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.0)
           .done()
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, HeightError> {
        let mut fields = src.struct_fields();
        let height = fields.trust_field().ok().ok_or(HeightError)?;
        fields.assert_done();
        Self::new(height).ok_or(HeightError)
    }
}

impl Primitive for DummyHeight {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(Self)
    }
}

impl Primitive for DummyNonZeroHeight {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(Self)
    }
}

// --- Commit impls ----
/*
impl Commit for Height {
    const VERBATIM_LEN: usize = 1;
    type Committed = Self;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.0)
    }
}

impl Commit for NonZeroHeight {
    const VERBATIM_LEN: usize = 1;
    type Committed = Self;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.0.get())
    }
}
*/


macro_rules! impl_cmp_ops {
    ($( $t:ty = $u:ty, )+) => {$(
        impl cmp::PartialEq<$u> for $t {
            #[inline(always)]
            fn eq(&self, other: &$u) -> bool {
                let this: u8 = (*self).into();
                let other: u8 = (*other).into();
                this == other
            }
        }

        impl cmp::PartialEq<$t> for $u {
            #[inline(always)]
            fn eq(&self, other: &$t) -> bool {
                let this: u8 = (*self).into();
                let other: u8 = (*other).into();
                this == other
            }
        }

        impl cmp::PartialOrd<$u> for $t {
            #[inline(always)]
            fn partial_cmp(&self, other: &$u) -> Option<cmp::Ordering> {
                let this: u8 = (*self).into();
                let other: u8 = (*other).into();
                this.partial_cmp(&other)
            }
        }

        impl cmp::PartialOrd<$t> for $u {
            #[inline(always)]
            fn partial_cmp(&self, other: &$t) -> Option<cmp::Ordering> {
                let this: u8 = (*self).into();
                let other: u8 = (*other).into();
                this.partial_cmp(&other)
            }
        }
    )+}
}

impl_cmp_ops! {
    Height = NonZeroHeight,
    Height = u8,
    NonZeroHeight = u8,
}

macro_rules! impl_fmt {
    ($( $t:ty, )+) => {$(
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.to_height().get().fmt(f)
            }
        }
    )+}
}

impl_fmt! {
    Height, HeightDyn,
    NonZeroHeight, NonZeroHeightDyn,
}
