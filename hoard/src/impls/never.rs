use super::*;

impl ValidateBlob for ! {
    type Error = !;

    const BLOB_LEN: usize = 0;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, !> {
        unsafe { Ok(blob.assume_valid()) }
    }
}
