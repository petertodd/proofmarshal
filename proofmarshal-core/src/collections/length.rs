//! Tree lengths.

use std::num::{NonZeroU8, NonZeroU64, NonZeroUsize};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::cmp;
use std::ops;
use std::hint;

use thiserror::Error;

use hoard::blob::{Bytes, BytesUninit};
use hoard::primitive::Primitive;

use crate::{unreachable_unchecked, impl_commit};
use crate::collections::height::{Height, NonZeroHeight};

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

impl From<InnerLength> for Length {
    fn from(len: InnerLength) -> Self {
        Self(len.get())
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
    pub const ZERO: Self = Length(0);

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
    /// # use proofmarshal_core::collections::length::Length;
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

    /// Splits the `Length` into left and right, if possible.
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::{InnerLength, NonZeroLength};
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .split(),
    ///            (NonZeroLength::new(0b10).unwrap(),
    ///             NonZeroLength::new(0b01).unwrap()));
    /// ```
    pub fn split(self) -> Result<(NonZeroLength, NonZeroLength), Option<Height>> {
        let len = NonZeroLength::new(self.get()).ok_or(None)?;
        let len = NonZeroLength::try_into_inner_length(len)?;
        Ok(len.split())
    }

    /// Returns true if a 2ⁿ height tree is contained within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::Length;
    /// # use proofmarshal_core::collections::height::Height;
    /// let len = Length(0b1010);
    ///
    /// assert!(!len.contains(Height::new(0).unwrap()));
    /// assert!(len.contains(Height::new(1).unwrap()));
    /// assert!(!len.contains(Height::new(2).unwrap()));
    /// assert!(len.contains(Height::new(3).unwrap()));
    /// ```
    pub fn contains(self, height: impl Into<Height>) -> bool {
        let height = height.into();
        match self.0 & (1 << height.get()) {
            0 => false,
            _ => true,
        }
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
        Self::new(1 << height.get())
             .unwrap_or_else(|| unsafe { unreachable_unchecked!() })
    }

    pub const fn get(self) -> NonZeroUsize {
        self.0
    }

    /// Checked addition.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::NonZeroLength;
    /// assert_eq!(NonZeroLength::MIN.checked_add(0),
    ///            Some(NonZeroLength::MIN));
    ///
    /// assert_eq!(NonZeroLength::MAX.checked_add(1),
    ///            None);
    /// ```
    pub fn checked_add(self, other: impl Into<Length>) -> Option<Self> {
        self.get().get().checked_add(other.into().get())
                        .and_then(Self::new)
    }

    #[track_caller]
    pub fn push_peak(self, right: impl Into<Height>) -> Result<InnerLength, Option<NonZeroHeight>> {
        let right = right.into();
        if self.min_height() > right {
            let right = NonZeroLength::from_height(right);
            let sum = self.0 | right.0;
            Ok(InnerLength(sum))
        } else if self.min_height() == right {
            let right_len = NonZeroLength::from_height(right);
            match self.get().get().checked_add(right_len.into()) {
                None => Err(None),
                Some(sum) if sum.is_power_of_two() => {
                    let height = u8::try_from(sum.trailing_zeros()).unwrap();

                    let height = unsafe { NonZeroU8::new_unchecked(height) };
                    let height = unsafe { NonZeroHeight::new_unchecked(height) };

                    Err(Some(height))
                },
                Some(sum) => {
                    unsafe {
                        Ok(InnerLength::new_unchecked(sum))
                    }
                }
            }
        } else {
            panic!("can't push: self.min_height() = {} < {}", self.min_height(), right)
        }
    }

    /// Tries to converts a `NonZeroLength` into an `InnerLength`.
    ///
    /// If this is *not* possible, returns the `Height` of the remaining tree.
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::{NonZeroLength, InnerLength};
    /// # use proofmarshal_core::collections::height::Height;
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
        let lsb = self.0.get().trailing_zeros() as u8;
        InnerLength::new(self.into())
            .ok_or(Height::new(lsb)
                          .unwrap_or_else(|| unsafe { unreachable_unchecked!() }))
    }

    /// Returns the minimum height tree within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .min_height(),
    ///            Height::new(0).unwrap());
    ///
    /// assert_eq!(InnerLength::new(0b1100000000000000000000000000000000000000000000000000000000000000).unwrap()
    ///                        .min_height(),
    ///            Height::new(62).unwrap());
    /// ```
    pub fn min_height(self) -> Height {
        let r = self.0.get().trailing_zeros() as u8;
        r.try_into()
         .unwrap_or_else(|_| unsafe { unreachable_unchecked!() })
    }

    /// Returns the maximum height tree within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .max_height(),
    ///            Height::new(1).unwrap());
    ///
    /// assert_eq!(InnerLength::new(0b1100000000000000000000000000000000000000000000000000000000000000).unwrap()
    ///                        .max_height(),
    ///            Height::new(63).unwrap());
    /// ```
    pub fn max_height(self) -> Height {
        let r = (usize::MAX.count_ones() - 1 - self.0.get().leading_zeros()) as u8;
        r.try_into()
         .unwrap_or_else(|_| unsafe { unreachable_unchecked!() })
    }

    /// Returns true if a 2ⁿ height tree is contained within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::NonZeroLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// let len = NonZeroLength::new(0b1010).unwrap();
    ///
    /// assert!(!len.contains(Height::new(0).unwrap()));
    /// assert!(len.contains(Height::new(1).unwrap()));
    /// assert!(!len.contains(Height::new(2).unwrap()));
    /// assert!(len.contains(Height::new(3).unwrap()));
    /// ```
    pub fn contains(self, height: impl Into<Height>) -> bool {
        Length::from(self).contains(height)
    }
}

impl InnerLength {
    /// The largest possible value.
    pub const MAX: Self = InnerLength(NonZeroLength::MAX.0);

    /// The smallest possible value.
    pub const MIN: Self = InnerLength(unsafe { NonZeroUsize::new_unchecked(0b11) });

    /// Creates a new `InnerLength`.
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
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
    /// # use proofmarshal_core::collections::length::{InnerLength, NonZeroLength};
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

    /// Checked push.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::convert::TryFrom;
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::{Height, NonZeroHeight};
    /// let left = InnerLength::new(0b110).unwrap();
    /// let right = Height::new(0).unwrap();
    /// assert_eq!(left.push_peak(right).unwrap(),
    ///            0b111);
    ///
    /// let left = InnerLength::new(0b11).unwrap();
    /// let right = Height::new(0).unwrap();
    /// assert_eq!(left.push_peak(right).unwrap_err(),
    ///            Some(NonZeroHeight::try_from(2).unwrap()));
    ///
    /// let left = InnerLength::MAX;
    /// let right = Height::new(0).unwrap();
    /// assert_eq!(left.push_peak(right).unwrap_err(),
    ///            None);
    /// ```
    ///
    /// ```should_panic
    /// # use std::convert::TryFrom;
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::{Height, NonZeroHeight};
    /// let left = InnerLength::MAX;
    /// let right = Height::new(1).unwrap();
    /// left.push_peak(right); // panics!
    /// ```
    #[track_caller]
    pub fn push_peak(self, right: impl Into<Height>) -> Result<Self, Option<NonZeroHeight>> {
        let left = NonZeroLength::from(self);
        left.push_peak(right)
    }

    /// Splits the `InnerLength` into left and right.
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::{InnerLength, NonZeroLength};
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .split(),
    ///            (NonZeroLength::new(0b10).unwrap(),
    ///             NonZeroLength::new(0b01).unwrap()));
    ///
    /// assert_eq!(InnerLength::new(0b110).unwrap()
    ///                        .split(),
    ///            (NonZeroLength::new(0b100).unwrap(),
    ///             NonZeroLength::new(0b010).unwrap()));
    ///
    /// assert_eq!(InnerLength::new(0b1111).unwrap()
    ///                        .split(),
    ///            (NonZeroLength::new(0b1100).unwrap(),
    ///             NonZeroLength::new(0b0011).unwrap()));
    /// ```
    pub const fn split(self) -> (NonZeroLength, NonZeroLength) {
        let mut mask = usize::MAX;
        loop {
            mask <<= 1;
            //debug_assert_ne!(mask, 0);

            let lhs = self.get() & mask;
            let rhs = self.get() & !mask;

            if lhs.count_ones().is_power_of_two() && rhs != 0 {
                let lhs = unsafe { NonZeroLength::new_unchecked(lhs) };
                let rhs = unsafe { NonZeroLength::new_unchecked(rhs) };
                break (lhs, rhs)
            }
        }
    }

    /// Returns the minimum height tree within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .min_height(),
    ///            Height::new(0).unwrap());
    ///
    /// assert_eq!(InnerLength::new(0b1100000000000000000000000000000000000000000000000000000000000000).unwrap()
    ///                        .min_height(),
    ///            Height::new(62).unwrap());
    /// ```
    pub fn min_height(self) -> Height {
        let r = self.get().trailing_zeros() as u8;
        r.try_into()
         .unwrap_or_else(|_| unsafe { unreachable_unchecked!() })
    }

    /// Returns the maximum height tree within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// assert_eq!(InnerLength::new(0b11).unwrap()
    ///                        .max_height(),
    ///            Height::new(1).unwrap());
    ///
    /// assert_eq!(InnerLength::new(0b1100000000000000000000000000000000000000000000000000000000000000).unwrap()
    ///                        .max_height(),
    ///            Height::new(63).unwrap());
    /// ```
    pub fn max_height(self) -> Height {
        let r = (usize::MAX.count_ones() - 1 - self.get().leading_zeros()) as u8;
        r.try_into()
         .unwrap_or_else(|_| unsafe { unreachable_unchecked!() })
    }

    /// Returns true if a 2ⁿ height tree is contained within this length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proofmarshal_core::collections::length::InnerLength;
    /// # use proofmarshal_core::collections::height::Height;
    /// let len = InnerLength::new(0b1010).unwrap();
    ///
    /// assert!(!len.contains(Height::new(0).unwrap()));
    /// assert!(len.contains(Height::new(1).unwrap()));
    /// assert!(!len.contains(Height::new(2).unwrap()));
    /// assert!(len.contains(Height::new(3).unwrap()));
    /// ```
    pub fn contains(self, height: impl Into<Height>) -> bool {
        Length::from(self).contains(height)
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
    type Error = NonZeroLengthError<usize>;

    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::new(len).ok_or(NonZeroLengthError(len))
    }
}

impl TryFrom<Length> for NonZeroLength {
    type Error = NonZeroLengthError<Length>;

    fn try_from(len: Length) -> Result<Self, Self::Error> {
        len.0.try_into().ok().ok_or(NonZeroLengthError(len))
    }
}

impl TryFrom<usize> for InnerLength {
    type Error = InnerLengthError;

    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::new(len).ok_or(InnerLengthError)
    }
}

impl TryFrom<NonZeroLength> for InnerLength {
    type Error = InnerLengthError;

    fn try_from(len: NonZeroLength) -> Result<Self, Self::Error> {
        Self::new(len.into()).ok_or(InnerLengthError)
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
pub struct NonZeroLengthError<T: fmt::Debug>(pub T);

impl Primitive for NonZeroLength {
    type DecodeBytesError = NonZeroLengthError<u64>;

    const BLOB_SIZE: usize = 8;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let n: u64 = self.0.get().try_into().unwrap();
        dst.write_bytes(&n.to_le_bytes())
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw_len = fields.trust_field::<u64>()?;
        fields.assert_done();
        let len = usize::try_from(raw_len).ok().ok_or(NonZeroLengthError(raw_len))?;
        let len = NonZeroUsize::new(len).ok_or(NonZeroLengthError(raw_len))?;
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

macro_rules! impl_bit_ops {
    ($( $t:ty => { $( $u:ty ),+ } ,)+) => {$(
        $(
            impl ops::BitAnd<$u> for $t {
                type Output = Length;

                #[inline(always)]
                fn bitand(self, other: $u) -> Self::Output {
                    let lhs: usize = self.into();
                    let rhs: usize = other.into();
                    Length(lhs & rhs)
                }
            }

            impl ops::BitXor<$u> for $t {
                type Output = Length;

                #[inline(always)]
                fn bitxor(self, other: $u) -> Self::Output {
                    let lhs: usize = self.into();
                    let rhs: usize = other.into();
                    Length(lhs ^ rhs)
                }
            }
        )+
    )+}
}

impl_bit_ops! {
    Length => {Length, NonZeroLength, InnerLength, usize },
    NonZeroLength => {Length, NonZeroLength, InnerLength, usize },
    InnerLength => {Length, NonZeroLength, InnerLength, usize },
}

macro_rules! impl_fmts {
    ($( $t:ty, )+) => {$(
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let this = usize::from(*self);
                this.fmt(f)
            }
        }

        impl fmt::Binary for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let this = usize::from(*self);
                this.fmt(f)
            }
        }
    )+}
}

impl_fmts! {
    Length,
    NonZeroLength,
    InnerLength,
}

impl_commit! {
    Length,
    NonZeroLength,
    InnerLength,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let l = InnerLength::new(0b11).unwrap();
        let (left, right) = l.split();
        assert_eq!(left, 2);
        assert_eq!(right, 1);
    }

    #[test]
    fn ops_bitand() {
        assert_eq!(Length(0) & Length(0),
                   0);
        assert_eq!(Length(0b11) & Length(0b1),
                   0b1);
        assert_eq!(Length(0b11) & 0b10,
                   0b10);
        assert_eq!(InnerLength::MAX & NonZeroLength::MAX,
                   Length::MAX);
    }
}
