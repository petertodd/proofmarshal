use std::mem;
use std::num;
use std::slice;

use thiserror::Error;

use leint::Le;

use super::*;

macro_rules! unsafe_impl_all_valid {
    ($($t:ty,)+) => {$(
        impl ValidateBlob for $t {
            const BLOB_LEN: usize = mem::size_of::<Self>();
            type Error = !;

            fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                unsafe { Ok(Blob::from(blob).assume_valid()) }
            }
        }
    )+}
}

unsafe_impl_all_valid! {
    (),
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
    Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

macro_rules! unsafe_impl_persist {
    ($($t:ty,)+) => {$(
        unsafe impl Persist for $t {}
    )+}
}

unsafe_impl_persist! {
    (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

#[non_exhaustive]
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("invalid bool blob")]
pub struct ValidateBoolError;

impl ValidateBlob for bool {
    type Error = ValidateBoolError;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let blob = Blob::from(blob);
        match blob.as_bytes() {
            [0] | [1] => unsafe { Ok(blob.assume_valid()) },
            _ => Err(ValidateBoolError),
        }
    }
}

#[non_exhaustive]
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("invalid nonzero blob")]
pub struct ValidateNonZeroError;

macro_rules! impl_nonzero {
    ($($t:ty,)+) => {$(
        impl ValidateBlob for $t {
            type Error = ValidateNonZeroError;
            const BLOB_LEN: usize = mem::size_of::<Self>();

            fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                let blob = Blob::from(blob);
                let mut bytes = [0; Self::BLOB_LEN];
                bytes.copy_from_slice(blob.as_bytes());

                if bytes == [0; Self::BLOB_LEN] {
                    Err(ValidateNonZeroError)
                } else {
                    unsafe { Ok(blob.assume_valid()) }
                }
            }
        }
    )+}
}

impl_nonzero! {
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
    num::NonZeroU16, num::NonZeroU32, num::NonZeroU64, num::NonZeroU128,
    num::NonZeroI16, num::NonZeroI32, num::NonZeroI64, num::NonZeroI128,
}
