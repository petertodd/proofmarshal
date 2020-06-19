use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::UninitArray;

use super::*;

impl<Z, T, const N: usize> Decode<Z> for [T; N]
where T: Decode<Z>
{
    fn decode_blob<'a>(blob: ValidBlob<Self>, zone: &Z) -> Self {
        let mut fields = blob.decode_fields(zone);
        let mut this = UninitArray::new();
        for i in 0 .. N {
            let item = unsafe { fields.decode_unchecked() };
            this.push(item);
        }
        fields.finish();
        this.done()
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
