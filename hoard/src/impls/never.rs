use super::*;

unsafe impl Persist for ! {
}

impl Scalar for ! {
    const BLOB_LAYOUT: BlobLayout = BlobLayout { size: 0, niche_start: 0, niche_end: 0, inhabited: false };
    type ScalarBlobError = !;

    fn validate_blob(blob: Blob<Self>) -> Result<ValidBlob<Self>, Self::ScalarBlobError> {
        Ok( unsafe { blob.assume_valid() } )
    }

    fn decode_blob(_blob: ValidBlob<Self>) -> Self {
        panic!()
    }

    fn try_deref_blob<'a>(_blob: ValidBlob<'a, Self>) -> Result<&'a Self, ValidBlob<'a, Self>> {
        panic!()
    }

    fn encode_blob<W: WriteBlob>(&self, _dst: W) -> Result<W::Ok, W::Error> {
        match *self {}
    }
}
