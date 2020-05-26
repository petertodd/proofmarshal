use super::*;

use crate::blob::*;
use crate::load::*;
use crate::save::*;

impl ValidateBlob for TryPile<'_, '_> {
    const BLOB_LEN: usize = 0;
    type Error = !;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.finish()) }
    }
}

impl<Q: Ptr> Decode<Q> for TryPile<'_, '_>
where Q::PersistZone: AsZone<Self>
{
    fn decode_blob(decoder: BlobDecoder<Q, Self>) -> Self {
        decoder.zone().as_zone().clone()
    }
}

impl<Q, R> Encode<Q, R> for TryPile<'_, '_> {
    type EncodePoll = ();
    fn init_encode(&self, _: &impl SavePtr) -> () {
    }
}

impl ValidateBlob for Pile<'_, '_> {
    const BLOB_LEN: usize = 0;
    type Error = !;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.finish()) }
    }
}

impl<Q: Ptr> Decode<Q> for Pile<'_, '_>
where Q::PersistZone: AsZone<Self>
{
    fn decode_blob(decoder: BlobDecoder<Q, Self>) -> Self {
        decoder.zone().as_zone().clone()
    }
}

impl<Q, R> Encode<Q, R> for Pile<'_, '_> {
    type EncodePoll = ();
    fn init_encode(&self, _: &impl SavePtr) -> () {
    }
}
