use super::*;

impl ValidateBlob for ! {
    const BLOB_LEN: usize = 0;
    type Error = !;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(Blob::from(blob).assume_valid()) }
    }
}

unsafe impl Persist for ! {
}
