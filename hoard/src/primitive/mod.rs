use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::zone::Ptr;

pub mod impls;

pub trait Primitive : 'static + Copy {
    type DecodeBytesError : 'static + std::error::Error;
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

impl<Z, P, T: Primitive> Saved<Z, P> for T {
    type Saved = T;
}

impl<T: Primitive> SaveDirty for T {
    type CleanPtr = !;
    type SaveDirtyPoll = PrimitiveSaveDirtyPoll<T>;

    fn init_save_dirty(&self) -> Self::SaveDirtyPoll {
        PrimitiveSaveDirtyPoll(*self)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PrimitiveSaveDirtyPoll<T>(T);

impl<T: Primitive> SaveDirtyPoll for PrimitiveSaveDirtyPoll<T> {
    type CleanPtr = !;
    type SavedBlob = T;

    fn save_dirty_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver
    {
        Ok(())
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        self.0
    }
}
