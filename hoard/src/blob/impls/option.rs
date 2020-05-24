use super::*;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateBlobOptionError<E: std::error::Error> {
    Discriminant,
    Value(E),
}

impl<T: ValidateBlob> ValidateBlob for Option<T> {
    const BLOB_LEN: usize = 1 + T::BLOB_LEN;
    type Error = ValidateBlobOptionError<T::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.field::<u8>().into_ok().as_value() {
            1 => {
                blob.field::<T>().map_err(ValidateBlobOptionError::Value)?;
                unsafe { Ok(blob.finish()) }
            },
            0 => {
                blob.field_bytes(T::BLOB_LEN);
                unsafe { Ok(blob.finish()) }
            },
            x => Err(ValidateBlobOptionError::Discriminant),
        }
    }
}
