use std::mem;
use std::num;
use std::slice;
use std::convert::TryInto;

use thiserror::Error;

use leint::Le;

use super::*;

#[derive(Debug)]
pub struct ScalarSavePoll<T>(T);

macro_rules! unsafe_impl_all_valid_persist {
    ($($t:ty,)+) => {$(
        impl BlobSize for $t {
            const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());
        }

        unsafe impl Persist for $t {}

        impl Scalar for $t {
            type Error = !;

            fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                unsafe { Ok(blob.assume_valid()) }
            }

            fn decode_blob(blob: ValidBlob<Self>) -> Self {
                blob.as_value().clone()
            }

            fn deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Ref<'a, Self> {
                blob.as_value().into()
            }

            fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error> {
                let src = unsafe {
                    slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>())
                };
                dst.write_bytes(src)
            }
        }
    )+}
}

unsafe_impl_all_valid_persist! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

#[non_exhaustive]
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("invalid bool blob")]
pub struct ValidateBoolError;

impl BlobSize for bool {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());
}

unsafe impl Persist for bool {}

impl Scalar for bool {
    type Error = ValidateBoolError;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.as_bytes() {
            [0] | [1] => unsafe { Ok(blob.assume_valid()) },
            _ => Err(ValidateBoolError),
        }
    }

    fn decode_blob(blob: ValidBlob<Self>) -> Self {
        blob.as_value().clone()
    }

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Ref<'a, Self> {
        blob.as_value().into()
    }

    fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error> {
        dst.write_bytes(&[if *self { 1 } else { 0 }])
    }
}

#[non_exhaustive]
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("invalid nonzero blob")]
pub struct ValidateNonZeroError;

macro_rules! unsafe_impl_nonzero_persist {
    ($($t:ty,)+) => {$(
        impl BlobSize for $t {
            const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());
        }

        unsafe impl Persist for $t {}

        impl Scalar for $t {
            type Error = ValidateNonZeroError;

            fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                let mut bytes = [0; Self::BLOB_LAYOUT.size()];
                bytes.copy_from_slice(blob.as_bytes());

                if bytes == [0; Self::BLOB_LAYOUT.size()] {
                    Err(ValidateNonZeroError)
                } else {
                    unsafe { Ok(blob.assume_valid()) }
                }
            }

            fn decode_blob(blob: ValidBlob<Self>) -> Self {
                blob.as_value().clone()
            }

            fn deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Ref<'a, Self> {
                blob.as_value().into()
            }

            fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error> {
                let src = unsafe {
                    slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>())
                };
                dst.write_bytes(src)
            }
        }
    )+}
}

unsafe_impl_nonzero_persist! {
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

macro_rules! impl_nonzero {
    ($($t:ty,)+) => {$(
        impl BlobSize for $t {
            const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());
        }

        impl Scalar for $t {
            type Error = ValidateNonZeroError;

            fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                let mut bytes = [0; Self::BLOB_LAYOUT.size()];
                bytes.copy_from_slice(blob.as_bytes());

                if bytes == [0; Self::BLOB_LAYOUT.size()] {
                    Err(ValidateNonZeroError)
                } else {
                    unsafe { Ok(blob.assume_valid()) }
                }
            }

            fn decode_blob(blob: ValidBlob<Self>) -> Self {
                todo!()
            }

            fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error> {
                dst.write_bytes(&self.get().to_le_bytes())
            }
        }
    )+}
}

impl_nonzero! {
    num::NonZeroU16, num::NonZeroU32, num::NonZeroU64, num::NonZeroU128,
    num::NonZeroI16, num::NonZeroI32, num::NonZeroI64, num::NonZeroI128,
}


macro_rules! impl_ints {
    ($($t:ty,)+) => {$(
        impl BlobSize for $t {
            const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());
        }

        impl Scalar for $t {
            type Error = !;

            fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                unsafe { Ok(blob.assume_valid()) }
            }

            fn decode_blob(blob: ValidBlob<Self>) -> Self {
                <$t>::from_le_bytes(blob.as_bytes().try_into().unwrap())
            }

            fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error> {
                dst.write_bytes(&self.to_le_bytes())
            }
        }
    )+}
}

impl_ints! {
    u16, u32, u64, u128,
    i16, i32, i64, i128,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bool_encode_blob() {
        assert_eq!(true.encode_blob(vec![]).into_ok(),  &[1]);
        assert_eq!(false.encode_blob(vec![]).into_ok(), &[0]);
    }
}
