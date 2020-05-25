use std::num;
use std::convert::TryFrom;

use crate::save::*;
use crate::load::{Decode, BlobDecoder};
use crate::blob::{Blob, ValidateBlob};

use leint::Le;

pub trait Primitive : Decode<()> + Encode<!, !> {
    fn encode_blob_bytes(&self) -> Vec<u8> {
        vec![].write_primitive(self).into_ok()
    }

    fn try_decode_blob_bytes(src: &[u8]) -> Result<Self, Self::Error> {
        let blob = Blob::<Self>::try_from(src).unwrap();
        let valid_blob = Self::validate_blob(blob.into())?;
        Ok(Self::decode_blob(BlobDecoder::new(valid_blob, &())))
    }
}

impl<T: Primitive, const N: usize> Primitive for [T; N] {}

impl<T: Primitive> Primitive for Option<T> {}

macro_rules! impl_primitive {
    ($( $t:ty, )+) => {$(
        impl Primitive for $t {}
    )+}
}

impl_primitive! {
    !, (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    u16, u32, u64, u128,
    i16, i32, i64, i128,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

pub fn test_option_decode(src: &[u8;2])
    -> Result<Option<bool>,
              crate::blob::impls::option::ValidateBlobOptionError<crate::blob::impls::scalars::ValidateBoolError>>
{
    Primitive::try_decode_blob_bytes(src)
}

pub fn test_array_decode(src: &[u8;4])
    -> Result<[bool; 4],
              crate::blob::impls::array::ValidateArrayError<crate::blob::impls::scalars::ValidateBoolError, 4>>
{
    Primitive::try_decode_blob_bytes(src)
}

pub fn test_array_decode2(src: &[u8;2])
    -> Option<[bool; 2]>
{
    Primitive::try_decode_blob_bytes(src)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option() {
        assert_eq!(Some(42u8).encode_blob_bytes(),
                   &[1, 42]);

        assert_eq!(<Option<u8> as Primitive>::try_decode_blob_bytes(&[1,42]).unwrap(),
                   Some(42u8));

        assert_eq!(<Option<bool> as Primitive>::try_decode_blob_bytes(&[1,1]).unwrap(),
                   Some(true));
    }

    #[test]
    fn test_ints() {
        assert_eq!(0x12345678_u32.encode_blob_bytes(),
                   &[0x78, 0x56, 0x34, 0x12]);

        assert_eq!(<u32 as Primitive>::try_decode_blob_bytes(&[0x78, 0x56, 0x34, 0x12]),
                   Ok(0x12345678));
    }
}
