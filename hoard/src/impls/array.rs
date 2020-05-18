use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

#[derive(Error, Debug, PartialEq, Eq)]
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

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_fields();
        for idx in 0 .. N {
            blob.validate::<T>().map_err(|err| ValidateArrayError { idx, err })?;
        }
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<Z, P, T, const N: usize> Load<Z, P> for [T; N]
where T: ValidateBlob + Load<Z, P>
{
    fn decode_blob_owned<'a>(mut loader: BlobLoader<'a, '_, Self, Z, P>) -> Self {
        let mut r: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for i in 0 .. N {
            let item = unsafe { loader.decode_unchecked() };
            initializer.push(item);
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        loader.assert_done();
        r2
    }
}

unsafe impl<T: Persist, const N: usize> Persist for [T; N] {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
