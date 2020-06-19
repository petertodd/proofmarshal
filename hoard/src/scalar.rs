//! Marshalling of basic scalar types.

use std::num;
use std::convert::TryFrom;

use crate::pointee::Pointee;
use crate::blob::{*, padding::{CheckPadding, IgnorePadding}};
use crate::refs::Ref;
use crate::load::{Load, Decode};
use crate::save::*;
use crate::writebytes::WriteBytes;

use leint::Le;

pub trait Scalar : Copy + BlobSize {
    type Error : std::error::Error + 'static + Send + Sync;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;

    fn decode_blob(blob: ValidBlob<Self>) -> Self;
    fn deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Ref<'a, Self> {
        Ref::Owned(Self::decode_blob(blob))
    }

    fn encode_blob<W: WriteBytes>(&self, dst: W) -> Result<W, W::Error>;
}

impl<V: Copy, T: Scalar> ValidateBlob<V> for T {
    type Error = T::Error;

    fn validate_blob<'a>(blob: Blob<'a, Self>, _: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        T::validate_blob(blob)
    }
}

impl<Z, T: Scalar> Load<Z> for T {
    fn decode_blob(blob: ValidBlob<Self>, _: &Z) -> Self::Owned {
        T::decode_blob(blob)
    }

    fn deref_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Ref<'a, Self> {
        T::deref_blob(blob)
    }
}

impl<Z, T: Scalar> Decode<Z> for T {
}

impl<Z, T: Scalar> Saved<Z> for T {
    type Saved = Self;
}

#[derive(Debug)]
pub struct ScalarSavePoll<T>(T);

impl<Y, Q, T: Scalar> SavePoll<Y, Q> for ScalarSavePoll<T> {
    type Target = T;

    fn save_blob<W: WriteBlob<Y, Q>>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}

impl<Y, Q, T: Scalar> Save<Y, Q> for T {
    type SavePoll = ScalarSavePoll<Self>;

    fn init_save(&self) -> Self::SavePoll {
        ScalarSavePoll(*self)
    }
}
