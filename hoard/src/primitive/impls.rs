use super::*;

use thiserror::Error;

use std::convert::TryFrom;
use std::mem;
use std::num;

impl Primitive for ! {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    #[inline(always)]
    fn encode_blob_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        match *self {}
    }

    #[inline(always)]
    fn decode_blob_bytes(_blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        panic!()
    }
}

impl Primitive for () {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    #[inline(always)]
    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    #[inline(always)]
    fn decode_blob_bytes(_blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(().into())
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[error("FIXME")]
pub struct DecodeBoolError;

impl Primitive for bool {
    const BLOB_SIZE: usize = 1;
    type DecodeBytesError = DecodeBoolError;

    #[inline(always)]
    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[if *self { 1 } else { 0 }])
    }

    #[inline(always)]
    fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        match blob[0] {
            0 => Ok(false.into()),
            1 => Ok(true.into()),
            _ => Err(DecodeBoolError),
        }
    }
}

macro_rules! impl_ints {
    ($($t:ty,)+) => {$(
        impl Primitive for $t {
            const BLOB_SIZE: usize = mem::size_of::<$t>();
            type DecodeBytesError = !;

            #[inline(always)]
            fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
                dst.write_bytes(&self.to_le_bytes())
            }

            #[inline(always)]
            fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
                let buf = TryFrom::try_from(&*blob).unwrap();
                Ok(Self::from_le_bytes(buf))
            }
        }
    )+}
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[derive(Error, Debug)]
#[non_exhaustive]
#[error("FIXME")]
pub struct DecodeNonZeroIntError;

macro_rules! impl_nonzero_ints {
    ($($n:ty => $t:ty, )+) => {$(
        impl Primitive for $t {
            const BLOB_SIZE: usize = mem::size_of::<$t>();
            type DecodeBytesError = DecodeNonZeroIntError;

            #[inline(always)]
            fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
                dst.write_bytes(&self.get().to_le_bytes())
            }

            #[inline(always)]
            fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
                let buf: [u8; mem::size_of::<$t>()] = TryFrom::try_from(&*blob).unwrap();

                Self::new(<$n>::from_le_bytes(buf)).ok_or(DecodeNonZeroIntError)
            }
        }
    )+}
}

impl_nonzero_ints! {
    u8 => num::NonZeroU8, u16 => num::NonZeroU16, u32 => num::NonZeroU32, u64 => num::NonZeroU64, u128 => num::NonZeroU128,
    i8 => num::NonZeroI8, i16 => num::NonZeroI16, i32 => num::NonZeroI32, i64 => num::NonZeroI64, i128 => num::NonZeroI128,
}
