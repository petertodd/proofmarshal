use super::*;

use thiserror::Error;

use std::convert::TryFrom;
use std::mem;

impl Primitive for ! {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    fn encode_blob_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        match *self {}
    }

    fn decode_blob_bytes(_blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        panic!()
    }
}

impl Primitive for () {
    const BLOB_SIZE: usize = 0;
    type DecodeBytesError = !;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[])
    }

    fn decode_blob_bytes(_blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        Ok(().into())
    }
}

#[derive(Error, Debug)]
#[non_exhaustive]
#[error("FIXME")]
pub struct DecodeBoolError;

impl Primitive for bool {
    const BLOB_SIZE: usize = 1;
    type DecodeBytesError = DecodeBoolError;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&[if *self { 1 } else { 0 }])
    }

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

            fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
                dst.write_bytes(&self.to_le_bytes())
            }

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

/*
macro_rules! impl_validate_for_scalars {
    ($($t:ty,)+) => {$(
        impl ValidateImpl for $t {
            type Ptr = !;
            type Error = !;
        }

        impl ValidateBlob<'_> for $t {
            type ValidatePoll = ();

            fn init_validate_blob(&self) -> () {
            }
        }
    )+}
}

impl_validate_for_scalars! {
    (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}
*/
