use super::*;

use std::error::Error;
use thiserror::Error;

use hoard::blob::*;
use hoard::load::*;

use super::flags::ValidateFlagsBlobError;

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateSumTreeDataBlobError<SumError: Error, PtrError: Error> {
    Flags(<Flags as ValidateBlob>::Error),
    Sum(SumError),
    Ptr(PtrError),
}

impl<T, S, P> ValidateBlob for SumTreeData<T, S, P>
where S: ValidateBlob,
      P: ValidateBlob,
{
    const BLOB_LEN: usize = <u8 as ValidateBlob>::BLOB_LEN +
                            <Digest as ValidateBlob>::BLOB_LEN +
                            <S as ValidateBlob>::BLOB_LEN +
                            <P as ValidateBlob>::BLOB_LEN;

    type Error = ValidateSumTreeDataBlobError<<S as ValidateBlob>::Error, <P as ValidateBlob>::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<Flags>().map_err(ValidateSumTreeDataBlobError::Flags)?;
        blob.field::<Digest>().into_ok();
        blob.field::<S>().map_err(ValidateSumTreeDataBlobError::Sum)?;
        blob.field::<P>().map_err(ValidateSumTreeDataBlobError::Ptr)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Z, T, S, P> Decode<Z> for SumTreeData<T, S, P>
where S: Decode<Z>,
      P: Decode<Z>,
{
    fn decode_blob(mut blob: BlobDecoder<Z, Self>) -> Self {
        unsafe {
            Self {
                marker: PhantomData,
                flags: blob.field_unchecked::<u8>().into(),
                tip_digest: blob.field_unchecked::<Digest>().into(),
                sum: blob.field_unchecked::<S>().into(),
                tip: blob.field_unchecked(),
            }
        }
    }
}

unsafe impl<T, S, P> Persist for SumTreeData<T, S, P>
where S: Persist, P: Persist {}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateSumTreeBlobError<SumError: Error, PtrError: Error, ZoneError: Error> {
    Data(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Zone(ZoneError),
    Height(<Height as ValidateBlob>::Error),
}

impl<T, S, P: Ptr, Z> ValidateBlob for SumTree<T, S, P, Z>
where S: ValidateBlob,
      P: ValidateBlob,
      Z: ValidateBlob,
{
    const BLOB_LEN: usize = <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <Z as ValidateBlob>::BLOB_LEN +
                            <Height as ValidateBlob>::BLOB_LEN;

    type Error = ValidateSumTreeBlobError<S::Error, P::Error, Z::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateSumTreeBlobError::Data)?;
        blob.field::<Z>().map_err(ValidateSumTreeBlobError::Zone)?;
        blob.field::<Height>().map_err(ValidateSumTreeBlobError::Height)?;
        unsafe { Ok(blob.finish()) }
    }
}

unsafe impl<T, S, P:Ptr, Z> Persist for SumTree<T, S, P, Z>
where S: Persist, P: Persist, Z: Persist {}

impl<Y, T, S, P: Ptr, Z> Decode<Y> for SumTree<T, S, P, Z>
where S: Decode<Y>,
      P: Decode<Y>,
      Z: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        unsafe {
            Self {
                data: blob.field_unchecked(),
                zone: blob.field_unchecked(),
                height: blob.field_unchecked(),
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateInnerBlobError<SumError: Error, PtrError: Error> {
    Left(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Right(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Height(<Height as ValidateBlob>::Error),
}

impl<T, S, P: Ptr> ValidateBlob for Inner<T, S, P>
where S: ValidateBlob,
      P: ValidateBlob,
{
    const BLOB_LEN: usize = <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <Height as ValidateBlob>::BLOB_LEN;

    type Error = ValidateInnerBlobError<S::Error, P::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateInnerBlobError::Left)?;
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateInnerBlobError::Right)?;
        blob.field::<Height>().map_err(ValidateInnerBlobError::Height)?;
        unsafe { Ok(blob.finish()) }
    }
}

unsafe impl<T, S, P: Ptr> Persist for Inner<T, S, P>
where S: Persist, P: Persist, {}

impl<Y, T, S, P: Ptr> Decode<Y> for Inner<T, S, P>
where S: Decode<Y>,
      P: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        unsafe {
            Self {
                left: ManuallyDrop::new(blob.field_unchecked()),
                right: ManuallyDrop::new(blob.field_unchecked()),
                height: blob.field_unchecked(),
            }
        }
    }
}
