use super::*;

impl BlobSize for ! {
    const BLOB_LAYOUT: BlobLayout = BlobLayout { size: 0, niche_start: 0, niche_end: 0, inhabited: false };
}

unsafe impl Persist for ! {
}

impl Scalar for ! {
    type Error = !;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(Blob::from(blob).assume_valid()) }
    }

    fn decode_blob(blob: ValidBlob<Self>) -> Self {
        panic!()
    }

    fn encode_blob<W: WriteBytes>(&self, _: W) -> Result<W, W::Error> {
        match *self {}
    }
}
