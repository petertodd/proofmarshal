use std::num::{NonZeroU64, NonZeroUsize};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::cmp;

use thiserror::Error;

use hoard::blob::{Bytes, BytesUninit};
use hoard::primitive::Primitive;

use crate::collections::mmr::{Height, NonZeroHeight};

pub trait ToLength {
    fn to_length(&self) -> Length;
}

pub trait ToNonZeroLength {
    fn to_nonzero_length(&self) -> NonZeroLength;
}

impl From<NonZeroLength> for Length {
    fn from(len: NonZeroLength) -> Self {
        Self(len.0.get())
    }
}

impl<T: ?Sized + ToNonZeroLength> ToLength for T {
    fn to_length(&self) -> Length {
        self.to_nonzero_length().into()
    }
}

pub trait ToInnerLength {
    fn to_inner_length(&self) -> InnerLength;
}

impl From<InnerLength> for NonZeroLength {
    fn from(len: InnerLength) -> Self {
        Self(len.0)
    }
}

impl<T: ?Sized + ToInnerLength> ToNonZeroLength for T {
    fn to_nonzero_length(&self) -> NonZeroLength {
        self.to_inner_length().into()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Length(pub usize);

impl Length {
    /// The largest possible value.
    pub const MAX: Self = Length(usize::MAX);

    /// The smallest possible value.
    pub const MIN: Self = Length(0);

    pub fn from_height(height: impl Into<Height>) -> Self {
        let height = height.into();
        Self(1 << height.get())
    }

    /// Returns the value as a primitive type.
    pub const fn get(self) -> usize {
        self.0
    }

    /// Checked addition.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::Length;
    /// assert_eq!(Length(0).checked_add(0),
    ///            Some(Length(0)));
    ///
    /// assert_eq!(Length::MAX.checked_add(1),
    ///            None);
    /// ```
    pub fn checked_add(self, other: impl Into<Self>) -> Option<Self> {
        self.0.checked_add(other.into().get())
              .map(Self)
    }
}

impl ToLength for Length {
    fn to_length(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroLength(NonZeroUsize);

impl ToNonZeroLength for NonZeroLength {
    fn to_nonzero_length(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InnerLength(NonZeroUsize);

impl ToInnerLength for InnerLength {
    fn to_inner_length(&self) -> Self {
        *self
    }
}

impl NonZeroLength {
    /// The largest possible value.
    pub const MAX: Self = Self(unsafe { NonZeroUsize::new_unchecked(usize::MAX) });

    /// The smallest possible value.
    pub const MIN: Self = Self(unsafe { NonZeroUsize::new_unchecked(1) });

    pub const fn new(len: usize) -> Option<Self> {
        match NonZeroUsize::new(len) {
            None => None,
            Some(n) => Some(Self(n)),
        }
    }

    pub const unsafe fn new_unchecked(len: usize) -> Self {
        Self(NonZeroUsize::new_unchecked(len))
    }

    pub fn from_height(height: impl Into<Height>) -> Self {
        let height = height.into();
        Self::new(1 << height.get()).unwrap()
    }

    pub const fn get(self) -> usize {
        self.0.get()
    }

    /// Checked addition.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::NonZeroLength;
    /// assert_eq!(NonZeroLength::MIN.checked_add(0),
    ///            Some(NonZeroLength::MIN));
    ///
    /// assert_eq!(NonZeroLength::MAX.checked_add(1),
    ///            None);
    /// ```
    pub fn checked_add(self, other: impl Into<Length>) -> Option<Self> {
        self.get().checked_add(other.into().get())
                  .and_then(Self::new)
    }

    /// Tries to converts a `NonZeroLength` into an `InnerLength`.
    ///
    /// If this is *not* possible, returns the `Height` of the remaining tree.
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::{NonZeroLength, InnerLength};
    /// # use proofmarshal_core::collections::perfecttree::height::Height;
    /// assert_eq!(NonZeroLength::new(0b11).unwrap()
    ///                          .try_into_inner_length(),
    ///            Ok(InnerLength::new(0b11).unwrap()));
    ///
    /// assert_eq!(NonZeroLength::new(0b1).unwrap()
    ///                          .try_into_inner_length(),
    ///            Err(Height::new(0).unwrap()));
    ///
    /// assert_eq!(NonZeroLength::new(0b100).unwrap()
    ///                          .try_into_inner_length(),
    ///            Err(Height::new(2).unwrap()));
    /// ```
    pub fn try_into_inner_length(self) -> Result<InnerLength, Height> {
        let lsb = self.get().trailing_zeros() as u8;
        InnerLength::new(self.get())
            .ok_or(Height::new(lsb).unwrap())
    }
}

impl InnerLength {
    /// Creates a new `InnerLength`.
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::InnerLength;
    /// assert_eq!(InnerLength::new(0), None);
    /// assert_eq!(InnerLength::new(0b1), None);
    /// assert_eq!(InnerLength::new(0b10), None);
    /// assert_eq!(InnerLength::new(0b11).unwrap().get(), 0b11);
    /// ```
    pub const fn new(len: usize) -> Option<Self> {
        if len.count_ones() >= 2 {
            Some(unsafe { Self::new_unchecked(len) })
        } else {
            None
        }
    }

    pub const unsafe fn new_unchecked(len: usize) -> Self {
        Self(NonZeroUsize::new_unchecked(len))
    }

    pub const fn get(self) -> usize {
        self.0.get()
    }

    /// Checked addition.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::{InnerLength, NonZeroLength};
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .checked_add(0b100),
    ///            Ok(InnerLength::new(0b111).unwrap()));
    ///
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .checked_add(0b1),
    ///            Err(Some(NonZeroLength::new(0b100).unwrap())));
    /// ```
    pub fn checked_add(self, other: impl Into<Length>) -> Result<Self, Option<NonZeroLength>> {
        if let Some(len) = self.get().checked_add(other.into().get()) {
            Self::new(len)
                 .ok_or(NonZeroLength::new(len))
        } else {
            Err(None)
        }
    }

    /// Split the `InnerLength` into the height of the smallest tree, and the remaining length.
    ///
    /// ```
    /// # use proofmarshal_core::collections::mmr::length::{InnerLength, NonZeroLength};
    /// # use proofmarshal_core::collections::perfecttree::height::Height;
    /// let (rhs_height, rest_len) = InnerLength::new(0b11).unwrap().split();
    /// assert_eq!(rhs_height, 0);
    /// assert_eq!(rest_len, 0b1);
    /// ```
    pub fn split(self) -> (Height, NonZeroLength) {
        let height: u8 = self.get().trailing_zeros().try_into().unwrap();
        let rest = self.get() & !(2 << height);
        (height.try_into().unwrap(),
         rest.try_into().unwrap())
    }
}

impl From<usize> for Length {
    fn from(len: usize) -> Self {
        Self(len)
    }
}

impl From<Length> for usize {
    fn from(len: Length) -> Self {
        len.0
    }
}

impl From<NonZeroLength> for usize {
    fn from(len: NonZeroLength) -> Self {
        len.0.get()
    }
}

impl From<InnerLength> for usize {
    fn from(len: InnerLength) -> Self {
        len.get()
    }
}

impl TryFrom<usize> for NonZeroLength {
    type Error = NonZeroLengthError;

    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::new(len).ok_or(NonZeroLengthError)
    }
}

impl TryFrom<usize> for InnerLength {
    type Error = InnerLengthError;

    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::new(len).ok_or(InnerLengthError)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DummyLength;

impl ToLength for DummyLength {
    fn to_length(&self) -> Length {
        panic!()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DummyNonZeroLength;

impl ToNonZeroLength for DummyNonZeroLength {
    fn to_nonzero_length(&self) -> NonZeroLength {
        panic!()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DummyInnerLength;

impl ToInnerLength for DummyInnerLength {
    fn to_inner_length(&self) -> InnerLength {
        panic!()
    }
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LengthDyn([()]);

impl ToLength for LengthDyn {
    fn to_length(&self) -> Length {
        self.0.len().into()
    }
}

impl fmt::Debug for LengthDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("LengthDyn")
         .field(&self.0.len())
         .finish()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroLengthDyn([()]);

impl ToNonZeroLength for NonZeroLengthDyn {
    fn to_nonzero_length(&self) -> NonZeroLength {
        unsafe {
            NonZeroLength::new_unchecked(self.0.len() as usize)
        }
    }
}

impl fmt::Debug for NonZeroLengthDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("NonZeroLengthDyn")
         .field(&self.0.len())
         .finish()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InnerLengthDyn([()]);

impl ToInnerLength for InnerLengthDyn {
    fn to_inner_length(&self) -> InnerLength {
        unsafe {
            InnerLength::new_unchecked(self.0.len() as usize)
        }
    }
}

impl fmt::Debug for InnerLengthDyn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("InnerLengthDyn")
         .field(&self.0.len())
         .finish()
    }
}


// ------- Primitive impls -------

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct InnerLengthError;

impl Primitive for InnerLength {
    type DecodeBytesError = InnerLengthError;

    const BLOB_SIZE: usize = 8;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let n: u64 = self.0.get().try_into().unwrap();
        dst.write_bytes(&n.to_le_bytes())
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let len = fields.trust_field::<u64>()?;
        fields.assert_done();
        let len = usize::try_from(len).ok().ok_or(InnerLengthError)?;
        Self::new(len).ok_or(InnerLengthError)
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct NonZeroLengthError;

impl Primitive for NonZeroLength {
    type DecodeBytesError = NonZeroLengthError;

    const BLOB_SIZE: usize = 8;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let n: u64 = self.0.get().try_into().unwrap();
        dst.write_bytes(&n.to_le_bytes())
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let len = fields.trust_field::<u64>()?;
        fields.assert_done();
        let len = usize::try_from(len).ok().ok_or(NonZeroLengthError)?;
        let len = NonZeroUsize::new(len).ok_or(NonZeroLengthError)?;
        Ok(Self(len))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct LengthError;

impl Primitive for Length {
    type DecodeBytesError = LengthError;

    const BLOB_SIZE: usize = 8;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let n: u64 = self.0.try_into().unwrap();
        dst.write_bytes(&n.to_le_bytes())
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let len = fields.trust_field::<u64>()?;
        fields.assert_done();

        let len = usize::try_from(len).ok().ok_or(LengthError)?;
        Ok(Self(len))
    }
}

impl Primitive for DummyLength {
    type DecodeBytesError = !;

    const BLOB_SIZE: usize = 0;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(Self)
    }
}

impl Primitive for DummyNonZeroLength {
    type DecodeBytesError = !;

    const BLOB_SIZE: usize = 0;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(Self)
    }
}

impl Primitive for DummyInnerLength {
    type DecodeBytesError = !;

    const BLOB_SIZE: usize = 0;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(Self)
    }
}

// ----- cmp impls ---------
macro_rules! impl_cmp_ops {
    ($( $t:ty : $u:ty, )+) => {$(
        impl cmp::PartialEq<$u> for $t {
            #[inline(always)]
            fn eq(&self, other: &$u) -> bool {
                let this: usize = (*self).into();
                let other: usize = (*other).into();
                this == other
            }
        }

        impl cmp::PartialEq<$t> for $u {
            #[inline(always)]
            fn eq(&self, other: &$t) -> bool {
                let this: usize = (*self).into();
                let other: usize = (*other).into();
                this == other
            }
        }

        impl cmp::PartialOrd<$u> for $t {
            #[inline(always)]
            fn partial_cmp(&self, other: &$u) -> Option<cmp::Ordering> {
                let this: usize = (*self).into();
                let other: usize = (*other).into();
                this.partial_cmp(&other)
            }
        }

        impl cmp::PartialOrd<$t> for $u {
            #[inline(always)]
            fn partial_cmp(&self, other: &$t) -> Option<cmp::Ordering> {
                let this: usize = (*self).into();
                let other: usize = (*other).into();
                this.partial_cmp(&other)
            }
        }
    )+}
}

impl_cmp_ops! {
    Length : NonZeroLength,
    Length : InnerLength,
    Length : usize,
    NonZeroLength : InnerLength,
    NonZeroLength : usize,
    InnerLength : usize,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let l = InnerLength::new(0b11).unwrap();
        dbg!(l.split());
    }
}
