use std::marker::PhantomData;

use crate::load::*;
use crate::blob::*;
use crate::pointee::Pointee;
use crate::ptr::{PtrClean, PtrBlob, AsZone};

pub mod impls;

pub trait Saver {
    type Error;

    type SrcPtr : PtrClean;
    type DstPtr : PtrBlob;

    fn try_save_ptr<P, T: ?Sized>(&mut self, ptr: P::Blob, metadata: T::Metadata)
            -> Result<Result<Self::DstPtr, T::SavePoll>, Self::Error>
        where T: SaveRef<Self::SrcPtr, Self::DstPtr>,
              P: PtrClean + Into<Self::SrcPtr>,
              <Self::SrcPtr as PtrClean>::Zone: AsZone<P::Zone>;


    fn save_blob_with<T: ?Sized, F>(&mut self, metadata: T::Metadata, f: F) -> Result<Self::DstPtr, Self::Error>
        where T: BlobDyn,
              F: for<'a> FnOnce(BytesUninit<'a, T>) -> Bytes<'a, T>;

    fn save_blob<T: ?Sized + BlobDyn>(&mut self, blob: &T) -> Result<Self::DstPtr, Self::Error> {
        self.save_blob_with(T::metadata(blob), |dst| {
            blob.encode_bytes(dst)
        })
    }
}

pub trait Save<SrcPtr, DstPtr = <SrcPtr as PtrClean>::Blob> {
    type SrcBlob : Blob;
    type DstBlob : Blob;
    type SavePoll : SavePoll<SrcPtr, DstPtr, DstBlob = Self::DstBlob>;

    fn init_save(&self) -> Self::SavePoll;

    fn init_save_from_blob(blob: &Self::SrcBlob) -> Self::SavePoll;

    fn init_save_from_bytes(blob: Bytes<'_, Self::SrcBlob>)
        -> Result<Self::SavePoll, <Self::SrcBlob as Blob>::DecodeBytesError>
    {
        todo!()
    }
}

pub trait SavePoll<SrcPtr, DstPtr> {
    type DstBlob : Blob;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = SrcPtr, DstPtr = DstPtr>;

    fn encode_blob(&self) -> Self::DstBlob;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        let blob = self.encode_blob();
        Blob::encode_bytes(&blob, dst)
    }
}

pub trait SaveRef<SrcPtr, DstPtr = <SrcPtr as PtrClean>::Blob> : Pointee {
    type SrcBlob : ?Sized + BlobDyn<Metadata = Self::Metadata>;
    type DstBlob : ?Sized + BlobDyn<Metadata = Self::Metadata>;
    type SavePoll : SaveRefPoll<SrcPtr, DstPtr, DstBlob = Self::DstBlob>;

    fn init_save_ref(&self) -> Self::SavePoll;

    fn init_save_ref_from_bytes(blob: Bytes<'_, Self::SrcBlob>)
        -> Result<Self::SavePoll, <Self::SrcBlob as BlobDyn>::DecodeBytesError>;
}

pub trait SaveRefPoll<SrcPtr, DstPtr> {
    type DstBlob : ?Sized + BlobDyn;

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = SrcPtr, DstPtr = DstPtr>;

    fn blob_metadata(&self) -> <Self::DstBlob as Pointee>::Metadata;
    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob>;
}

impl<Q, R, T: Save<Q, R>> SaveRef<Q, R> for T {
    type SrcBlob = T::SrcBlob;
    type DstBlob = T::DstBlob;
    type SavePoll = T::SavePoll;

    fn init_save_ref(&self) -> Self::SavePoll {
        self.init_save()
    }

    fn init_save_ref_from_bytes(blob: Bytes<'_, Self::SrcBlob>)
        -> Result<Self::SavePoll, <Self::SrcBlob as BlobDyn>::DecodeBytesError>
    {
        todo!()
    }
}

impl<Q, R, T: SavePoll<Q, R>> SaveRefPoll<Q, R> for T {
    type DstBlob = T::DstBlob;

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
    {
        self.save_poll(saver)
    }

    fn blob_metadata(&self) -> () {
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        self.encode_blob_bytes(dst)
    }
}
