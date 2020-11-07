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
    type Zone = ();
    type Blob = Self;

    fn load(this: Self, _: &()) -> Self {
        this.into()
    }
}

impl<Q, R, T: Primitive> Save<Q, R> for T {
    type SavePoll = PrimitiveSavePoll<T>;
    type SrcBlob = T;
    type DstBlob = T;

    fn init_save(&self) -> Self::SavePoll {
        PrimitiveSavePoll(*self)
    }

    fn init_save_from_blob(this: &Self) -> Self::SavePoll {
        PrimitiveSavePoll(*this)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PrimitiveSavePoll<T>(T);

impl<Q, R, T: Primitive> SavePoll<Q, R> for PrimitiveSavePoll<T> {
    type DstBlob = T;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver
    {
        Ok(())
    }

    fn encode_blob(&self) -> Self::DstBlob {
        self.0
    }
}
