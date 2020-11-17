//! Serialization of types that never have internal pointers.

use std::marker::PhantomData;

use crate::blob::*;
use crate::load::Load;
use crate::save::{Save, SavePoll, Saver};

pub mod impls;

pub trait Primitive : 'static + Copy {
    type DecodeBytesError : 'static + std::error::Error + Send;
    const BLOB_SIZE: usize;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self>;
    fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError>;
}

impl<T: Primitive> Blob for T {
    const SIZE: usize = T::BLOB_SIZE;
    type DecodeBytesError = T::DecodeBytesError;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        self.encode_blob_bytes(dst)
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        Self::decode_blob_bytes(blob)
            .map(|this| this.into())
    }
}

impl<T: Primitive> Load for T {
    type PtrClean = !;
    type Zone = ();
    type Blob = Self;

    fn load_maybe_valid(this: MaybeValid<&Self>, _: &()) -> MaybeValid<Self> {
        this.trust().clone().into()
    }
}

impl<Q, T: Primitive> Save<Q> for T {
    type SavePoll = PrimitiveSavePoll<Q, T>;
    type DstBlob = T;

    fn init_save(&self) -> Self::SavePoll {
        PrimitiveSavePoll {
            marker: PhantomData,
            value: *self,
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PrimitiveSavePoll<Q, T> {
    marker: PhantomData<fn(Q)>,
    value: T,
}

impl<Q, T: Primitive> SavePoll for PrimitiveSavePoll<Q, T> {
    type SrcPtr = !;
    type DstPtr = Q;
    type DstBlob = T;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver
    {
        Ok(())
    }

    fn encode_blob(&self) -> Self::DstBlob {
        self.value
    }
}
