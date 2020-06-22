//! Marshalling of basic scalar types.

use std::convert::TryFrom;
use std::fmt;
use std::num;
use std::marker::PhantomData;

use crate::pointee::Pointee;
use crate::blob::{*, padding::{CheckPadding, IgnorePadding}};
use crate::refs::Ref;
use crate::load::*;
use crate::save::*;
use crate::zone::*;

use leint::Le;

pub trait Scalar : Copy {
    const SCALAR_BLOB_LAYOUT: BlobLayout;

    type Error : std::error::Error + 'static + Send + Sync;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;

    fn decode_blob(blob: ValidBlob<Self>) -> Self;

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Err(blob)
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W, W::Error>;
}

impl<T: Scalar> BlobSize for T {
    const BLOB_LAYOUT: BlobLayout = T::SCALAR_BLOB_LAYOUT;
}


impl<V, T: Scalar> ValidateBlob<V> for T {
    type Error = T::Error;

    fn validate_blob<'a>(blob: Blob<'a, Self>, _: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        T::validate_blob(blob)
    }
}

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

#[derive(Debug)]
pub struct ScalarEncodePoll<Y, T> {
    marker: PhantomData<fn(Y)>,
    value: T,
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
