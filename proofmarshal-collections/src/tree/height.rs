use std::cmp;
use std::convert::TryFrom;
use std::fmt;
use std::mem;
use std::num::NonZeroU8;
use std::ops;

use thiserror::Error;

// use hoard::pointee::{Metadata, MetadataKind};
use hoard::load::*;
use hoard::save::*;
use hoard::primitive::*;

use proofmarshal_core::commit::{Digest, Commit, Verbatim, WriteVerbatim};

/// The height of a perfect binary tree.
///
/// Valid range: `0 ..= 63`
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(u8);

impl fmt::Debug for Height {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct TryFromIntError;

/// The height of an inner node in a perfect binary tree.
///
/// Valid range: `1 ..= 63`
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroHeight(NonZeroU8);

impl fmt::Debug for NonZeroHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynHeight([()]);

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynNonZeroHeight([()]);

impl fmt::Debug for DynHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.len().fmt(f)
    }
}

impl fmt::Debug for DynNonZeroHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.len().fmt(f)
    }
}

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

    #[inline(always)]
    pub const unsafe fn new_unchecked(n: u8) -> Self {
        Self(n)
    }

    #[inline(always)]
    pub fn len(self) -> usize {
        self.assert_valid();
        1 << self.0
    }

    #[inline(always)]
    pub fn get(self) -> u8 {
        self.0

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

/*

hoard::impl_encode_for_primitive!(Height, |this, dst| {
    dst.write_bytes(&[this.0])?
        .finish()
});

hoard::impl_decode_for_primitive!(Height);

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range")]
#[non_exhaustive]
pub struct ValidateHeightError;

impl ValidateBlob for Height {
    type Error = ValidateHeightError;

    #[inline]
    fn validate<'a, V>(blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.validate_bytes(|blob| {
            if blob[0] <= Self::MAX {
                Ok(unsafe { blob.assume_valid() })
            } else {
                Err(ValidateHeightError)
            }
        })
    }
}

impl Primitive for Height {}

hoard::impl_encode_for_primitive!(NonZeroHeight, |this, dst| {
    dst.write_bytes(&[this.0.get()])?
        .finish()
});

hoard::impl_decode_for_primitive!(NonZeroHeight);

impl ValidateBlob for NonZeroHeight {
    type Error = ValidateHeightError;

    #[inline]
    fn validate<'a, V>(blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.validate_bytes(|blob| {
            if 0 < blob[0] && blob[0] <= Height::MAX {
                Ok(unsafe { blob.assume_valid() })
            } else {
                Err(ValidateHeightError)
            }
        })
    }
}

impl Primitive for NonZeroHeight {}

impl Metadata for NonZeroHeight {
    #[inline]
    fn kind(&self) -> MetadataKind {
        MetadataKind::Len(self.0.get() as u64)
    }
}

impl Metadata for Height {
    #[inline]
    fn kind(&self) -> MetadataKind {
        MetadataKind::Len(self.0 as u64)
    }
}
*/

impl TryFrom<u8> for Height {
    type Error = TryFromIntError;
    #[inline]
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        Self::new(n)
    }
}

impl TryFrom<NonZeroU8> for Height {
    type Error = TryFromIntError;
    #[inline]
    fn try_from(n: NonZeroU8) -> Result<Self, Self::Error> {
        Self::new(n.get())
    }
}

impl TryFrom<usize> for Height {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        if n <= Height::MAX as usize {
            Ok(Height::new(n as u8).unwrap())
        } else {
            Err(TryFromIntError)
        }
    }
}


impl TryFrom<Height> for NonZeroHeight {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(n: Height) -> Result<Self, Self::Error> {
        NonZeroU8::new(n.0).map(|n| NonZeroHeight(n))
            .ok_or(TryFromIntError)
    }
}

impl TryFrom<usize> for NonZeroHeight {
    type Error = TryFromIntError;
    #[inline]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        let height = Height::try_from(n)?;
        NonZeroHeight::try_from(height)
    }
}

impl From<Height> for u8 {
    #[inline]
    fn from(height: Height) -> u8 {
        height.0
    }
}

impl From<Height> for usize {
    #[inline]
    fn from(height: Height) -> usize {
        height.0 as usize
    }
}

impl From<NonZeroHeight> for Height {
    #[inline]
    fn from(height: NonZeroHeight) -> Height {
        Self(height.0.get())
    }
}

impl From<NonZeroHeight> for u8 {
    #[inline]
    fn from(height: NonZeroHeight) -> u8 {
        height.0.get()
    }
}

impl From<NonZeroHeight> for usize {
    #[inline]
    fn from(height: NonZeroHeight) -> usize {
        height.0.get() as usize
    }
}

impl cmp::PartialEq<u8> for Height {
    fn eq(&self, rhs: &u8) -> bool {
        self.0 == *rhs
    }
}
impl cmp::PartialEq<Height> for u8 {
    fn eq(&self, rhs: &Height) -> bool {
        rhs == self
    }
}

impl cmp::PartialOrd<u8> for Height {
    fn partial_cmp(&self, rhs: &u8) -> Option<cmp::Ordering> {
        self.0.partial_cmp(rhs)
    }
}
impl cmp::PartialOrd<Height> for u8 {
    fn partial_cmp(&self, rhs: &Height) -> Option<cmp::Ordering> {
        self.partial_cmp(&rhs.0)
    }
}

pub unsafe trait GetHeight {
    fn get(&self) -> Height;
}

unsafe impl GetHeight for DynHeight {
    #[inline]
    fn get(&self) -> Height {
        Height::try_from(self.0.len()).expect("invalid height")
    }
}

unsafe impl GetHeight for DynNonZeroHeight {
    #[inline]
    fn get(&self) -> Height {
        NonZeroHeight::try_from(self.0.len()).expect("invalid height").into()
    }
}

unsafe impl GetHeight for Height {
    #[inline]
    fn get(&self) -> Height {
        *self
    }
}

unsafe impl GetHeight for NonZeroHeight {
    #[inline]
    fn get(&self) -> Height {
        Height::from(*self)
    }
}

unsafe impl GetHeight for () {
    #[inline]
    fn get(&self) -> Height {
        panic!()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range: {0}")]
pub struct LoadHeightError(u8);

impl Load for Height {
    type Error = LoadHeightError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

impl<R> Saved<R> for Height {
    type Saved = Self;
}
impl<Q, R> Save<'_, Q, R> for Height {
    type State = ();

    fn init_save_state(&self) -> Self::State {}

    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut Self::State, dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        <Self as Save<Q,R>>::encode_blob(self, state, dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&[self.0])?
           .done()
    }
}
impl Primitive for Height {}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range: {0}")]
pub struct LoadNonZeroHeightError(u8);

impl Load for NonZeroHeight {
    type Error = LoadNonZeroHeightError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

impl<R> Saved<R> for NonZeroHeight {
    type Saved = Self;
}
impl<Q, R> Save<'_, Q, R> for NonZeroHeight {
    type State = ();

    fn init_save_state(&self) -> Self::State {}

    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut Self::State, dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        <Self as Save<Q,R>>::encode_blob(self, state, dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&[self.0.get()])?
           .done()
    }
}
impl Primitive for NonZeroHeight {}
