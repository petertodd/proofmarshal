//! Marshalling of basic scalar types.

use std::convert::TryFrom;
use std::fmt;
use std::num;
use std::marker::PhantomData;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::refs::Ref;
use crate::load::*;
use crate::save::*;
use crate::ptr::*;

use leint::Le;

pub trait Scalar : Copy {
    const BLOB_LAYOUT: BlobLayout;
    type ScalarBlobError : std::error::Error + 'static + Send + Sync;

    fn validate_blob(blob: Blob<Self>) -> Result<ValidBlob<Self>, Self::ScalarBlobError>;

    fn decode_blob(blob: ValidBlob<Self>) -> Self;

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Err(blob)
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error>;
}

unsafe impl<T: Scalar> ValidateBlob for T {
    type BlobError = T::ScalarBlobError;

    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(T::BLOB_LAYOUT)
    }

    fn validate_blob(blob: Blob<Self>, _: bool) -> Result<ValidBlob<Self>, Self::BlobError> {
        T::validate_blob(blob)
    }
}

impl<T: Scalar> Load for T {
    type Ptr = !;

    fn decode_blob(blob: ValidBlob<Self>, _: &()) -> Self {
        T::decode_blob(blob)
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, _: &()) -> Result<&'a Self, ValidBlob<'a, Self>> {
        T::try_deref_blob(blob)
    }
}

impl<Q: Ptr, T: Scalar> Saved<Q> for T {
    type Saved = T;
}

impl<Q: Ptr, T: Scalar> Save<Q> for T {
    type SavePoll = ScalarSavePoll<Q, T>;

    fn init_save(&self) -> Self::SavePoll {
        ScalarSavePoll {
            marker: PhantomData,
            value: *self,
        }
    }
}

#[derive(Debug)]
pub struct ScalarSavePoll<Q, T> {
    marker: PhantomData<Q>,
    value: T,
}

impl<Q: Ptr, T: Scalar> EncodeBlob for ScalarSavePoll<Q, T> {
    type Target = T;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        self.value.encode_blob(dst)
    }
}

impl<Q: Ptr, T: Scalar> SavePoll for ScalarSavePoll<Q, T> {
    type SrcPtr = !;

    type DstPtr = Q;

    fn save_poll<S: Saver>(&mut self, _saver: &mut S) -> Result<(), S::Error> {
        Ok(())
    }
}

/*
impl<T: Scalar> Decode for T {
    type Zone = ();
    type Ptr = !;

    fn decode_blob(blob: ValidBlob<Self>, _: &()) -> Self {
        T::decode_blob(blob)
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, _: &()) -> Result<&'a Self, ValidBlob<'a, Self>> {
        T::try_deref_blob(blob)
    }
}

impl<Y: Zone, T: Scalar> Encode<Y> for T {
    type Encoded = Self;
    type EncodePoll = ScalarEncodePoll<Y, T>;

    fn init_encode(&self) -> Self::EncodePoll {
        ScalarEncodePoll {
            marker: PhantomData,
            value: self.clone(),
        }
    }
}


impl<Y: Zone, T: Scalar> EncodePoll for ScalarEncodePoll<Y, T> {
    type SrcZone = ();
    type SrcPtr = !;
    type DstZone = Y;
    type Target = T;

    fn encode_poll<S: Saver>(&mut self, _: &mut S) -> Result<(), S::Error> {
        Ok(())
    }
}
*/
