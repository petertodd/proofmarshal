use std::borrow::{Borrow, ToOwned};
use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::mem;
use std::num::NonZeroU8;
use std::ops;
use std::slice;

use thiserror::Error;

use hoard::blob::*;
use hoard::load::*;
use hoard::save::*;
use hoard::primitive::*;

use proofmarshal_core::commit::{Digest, Commit, Verbatim, WriteVerbatim};

/// The height of a perfect binary tree.
///
/// Valid range: `0 ..= 63`
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Height(u8);

impl fmt::Debug for Height {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("height out of range: {0:?}")]
#[non_exhaustive]
pub struct HeightError<T: std::fmt::Debug>(T);

impl Height {
    pub const MAX: u8 = 63;

    #[inline(always)]
    fn assert_valid(&self) {
        assert!(self.0 <= Self::MAX);
    }

    #[inline(always)]
    pub fn new(n: u8) -> Result<Self, HeightError<u8>> {
        if n <= Self::MAX {
            Ok(Self(n))
        } else {
            Err(HeightError(n))
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

impl TryFrom<u8> for Height {
    type Error = HeightError<u8>;
    #[inline]
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        Self::new(n)
    }
}

impl TryFrom<NonZeroU8> for Height {
    type Error = HeightError<NonZeroU8>;
    #[inline]
    fn try_from(n: NonZeroU8) -> Result<Self, Self::Error> {
        Self::new(n.into())
             .ok().ok_or(HeightError(n))
    }
}

impl TryFrom<usize> for Height {
    type Error = HeightError<usize>;

    #[inline]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        if n <= Height::MAX as usize {
            Ok(Height::new(n as u8).unwrap())
        } else {
            Err(HeightError(n))
        }
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

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("nonzero height out of range: {0:?}")]
#[non_exhaustive]
pub struct NonZeroHeightError<T: fmt::Debug>(T);

impl<T: fmt::Debug, U: fmt::Debug> From<HeightError<T>> for NonZeroHeightError<U>
where T: Into<U>
{
    fn from(err: HeightError<T>) -> Self {
        Self(err.0.into())
    }
}

/// The height of an inner node in a perfect binary tree.
///
/// Valid range: `1 ..= 63`
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct NonZeroHeight(NonZeroU8);

impl fmt::Debug for NonZeroHeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl NonZeroHeight {
    pub const MAX: u8 = 63;

    #[inline(always)]
    pub fn new(n: NonZeroU8) -> Result<Self, NonZeroHeightError<NonZeroU8>> {
        if n.get() <= Height::MAX {
            Ok(Self(n))
        } else {
            Err(NonZeroHeightError(n))
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

    #[inline(always)]
    pub fn get(self) -> NonZeroU8 {
        self.0
    }
}


impl TryFrom<Height> for NonZeroHeight {
    type Error = NonZeroHeightError<Height>;

    #[inline]
    fn try_from(n: Height) -> Result<Self, Self::Error> {
        NonZeroU8::new(n.0).map(|n| NonZeroHeight(n))
            .ok_or(NonZeroHeightError(n))
    }
}

impl TryFrom<usize> for NonZeroHeight {
    type Error = NonZeroHeightError<usize>;
    #[inline]
    fn try_from(n: usize) -> Result<Self, Self::Error> {
        let height = Height::try_from(n)?;
        NonZeroHeight::try_from(height)
            .ok().ok_or(NonZeroHeightError(n))
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

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct HeightDyn([()]);

impl fmt::Debug for HeightDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.len().fmt(f)
    }
}

impl Borrow<HeightDyn> for Height {
    fn borrow(&self) -> &HeightDyn {
        unsafe {
            let slice: &[()] = slice::from_raw_parts(&(), (*self).into());
            mem::transmute(slice)
        }
    }
}

impl ToOwned for HeightDyn {
    type Owned = Height;
    fn to_owned(&self) -> Self::Owned {
        self.0.len().try_into()
              .expect("height to be valid")
    }
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonZeroHeightDyn([()]);

impl fmt::Debug for NonZeroHeightDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.len().fmt(f)
    }
}

impl Borrow<NonZeroHeightDyn> for NonZeroHeight {
    fn borrow(&self) -> &NonZeroHeightDyn {
        unsafe {
            let slice: &[()] = slice::from_raw_parts(&(), (*self).into());
            mem::transmute(slice)
        }
    }
}

impl ToOwned for NonZeroHeightDyn {
    type Owned = NonZeroHeight;
    fn to_owned(&self) -> Self::Owned {
        self.0.len().try_into()
              .expect("non-zero height to be valid")
    }
}

pub trait ToHeight {
    fn to_height(&self) -> Height;
}

impl ToHeight for Height {
    fn to_height(&self) -> Height {
        *self
    }
}

impl ToHeight for HeightDyn {
    fn to_height(&self) -> Height {
        self.to_owned()
    }
}

pub trait ToNonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight;
}

impl<T: ?Sized + ToNonZeroHeight> ToHeight for T {
    fn to_height(&self) -> Height {
        self.to_nonzero_height().into()
    }
}

impl ToNonZeroHeight for NonZeroHeight {
    fn to_nonzero_height(&self) -> NonZeroHeight {
        (*self).into()
    }
}

impl ToNonZeroHeight for NonZeroHeightDyn {
    fn to_nonzero_height(&self) -> NonZeroHeight {
        self.to_owned().into()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range: {0}")]
pub struct ValidateBlobHeightError(u8);

impl ValidateBlob for Height {
    type Error = ValidateBlobHeightError;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.field_bytes(1)[0] {
            0 ..= Self::MAX => unsafe { Ok(blob.finish()) },
            x => Err(ValidateBlobHeightError(x)),
        }
    }
}

impl<Z> Decode<Z> for Height {
    fn decode_blob(blob: hoard::load::BlobDecoder<Z, Self>) -> Self {
        blob.to_value().clone()
    }
}

unsafe impl Persist for Height {}

impl<R> Encoded<R> for Height {
    type Encoded = Self;
}

impl<Q, R> Encode<'_, Q, R> for Height {
    type State = ();
    fn init_encode_state(&self) -> () {}

    fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_primitive(&self.0)?
           .done()
    }
}

impl Primitive for Height {}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("out of range: {0}")]
pub struct ValidateBlobNonZeroHeightError(u8);

impl ValidateBlob for NonZeroHeight {
    type Error = ValidateBlobNonZeroHeightError;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.field_bytes(1)[0] {
            1 ..= Self::MAX => unsafe { Ok(blob.finish()) },
            x => Err(ValidateBlobNonZeroHeightError(x)),
        }
    }
}

impl<Z> Decode<Z> for NonZeroHeight {
    fn decode_blob(blob: hoard::load::BlobDecoder<Z, Self>) -> Self {
        blob.to_value().clone()
    }
}

unsafe impl Persist for NonZeroHeight {}

impl<R> Encoded<R> for NonZeroHeight {
    type Encoded = Self;
}

impl<Q, R> Encode<'_, Q, R> for NonZeroHeight {
    type State = ();
    fn init_encode_state(&self) -> () {}

    fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_primitive(&self.0)?
           .done()
    }
}

impl Primitive for NonZeroHeight {}
