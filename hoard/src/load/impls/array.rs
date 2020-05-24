use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

impl<Z, T, const N: usize> Decode<Z> for [T; N]
where T: Decode<Z>
{
    fn decode_blob<'a>(mut blob: BlobDecoder<Z, Self>) -> Self {
        let mut r: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for i in 0 .. N {
            let item = unsafe { blob.field_unchecked() };
            initializer.push(item);
        }
        blob.finish();

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        r2
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;
    use crate::impls::scalars::ValidateBoolError;

    #[test]
    fn validate_blob() {
        let blob = Blob::<[bool; 4]>::try_from(&[0,1,0,1][..]).unwrap();
        let blob = ValidateBlob::validate_blob(blob.into()).unwrap();
        assert_eq!(blob.as_value(), &[false, true, false, true]);

        let blob = Blob::<[bool; 4]>::try_from(&[0,1,0,3][..]).unwrap();
        let err = ValidateBlob::validate_blob(blob.into()).unwrap_err();
        assert_eq!(err.idx, 3);
        assert_eq!(err.err, ValidateBoolError);
    }
}
*/
