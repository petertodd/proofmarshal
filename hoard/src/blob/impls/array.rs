use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

unsafe impl<T: Persist, const N: usize> Persist for [T; N] {}

#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("array validation failed at index {idx}: {err}")]
pub struct ValidateArrayError<E: Error, const N: usize> {
    idx: usize,
    err: E,
}

impl<E: Error, const N: usize> From<ValidateArrayError<E, N>> for !
where E: Into<!>
{
    fn from(err: ValidateArrayError<E,N>) -> ! {
        err.err.into()
    }
}

impl<T: ValidateBlob, const N: usize> ValidateBlob for [T; N] {
    type Error = ValidateArrayError<T::Error, N>;
    const BLOB_LEN: usize = T::BLOB_LEN * N;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        for idx in 0 .. N {
            blob.field::<T>().map_err(|err| ValidateArrayError { idx, err })?;
        }
        unsafe { Ok(blob.finish()) }
    }
}
