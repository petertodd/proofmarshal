use std::marker::PhantomData;

use crate::load::*;
use crate::blob::*;
use crate::pointee::Pointee;
use crate::ptr::{Ptr, PtrClean, PtrBlob, AsZone};

pub mod impls;

pub trait Saver {
    type Error;

    type SrcPtr : PtrClean;

    type DstPtr : PtrBlob;

    fn save_ptr<T: ?Sized>(&mut self, ptr: Self::SrcPtr, metadata: T::Metadata)
        -> Result<Result<Self::DstPtr, T::SaveRefPoll>, Self::Error>
        where T: SaveRef<Self::DstPtr>,
              <Self::SrcPtr as Ptr>::Zone: AsZone<T::Zone>;

    fn poll<T: ?Sized>(&mut self, poll: &mut T) -> Result<(), Self::Error>
        where T: SaveRefPoll<DstPtr = Self::DstPtr>,
              Self::SrcPtr: From<T::SrcPtr>,
              <Self::SrcPtr as Ptr>::Zone: AsZone<<T::SrcPtr as Ptr>::Zone>;

    fn poll_ref<T: ?Sized>(&mut self, poll: &mut T) -> Result<Self::DstPtr, Self::Error>
        where T: SaveRefPoll<DstPtr = Self::DstPtr>,
              Self::SrcPtr: From<T::SrcPtr>,
              <Self::SrcPtr as Ptr>::Zone: AsZone<<T::SrcPtr as Ptr>::Zone>;
}

pub trait Save<DstPtr> : Load {
    type DstBlob : Blob;

    type SavePoll : SavePoll<SrcPtr = <Self::Ptr as Ptr>::Clean, DstPtr = DstPtr, DstBlob = Self::DstBlob>;

    fn init_save(&self) -> Self::SavePoll;

    fn init_save_from_bytes(bytes: Bytes<'_, Self::Blob>, zone: &Self::Zone)
        -> Result<Self::SavePoll, <Self::Blob as Blob>::DecodeBytesError>
    {
        let this = Self::load_bytes(bytes, zone)?.trust();
        Ok(this.init_save())
    }
}

pub trait SavePoll {
    type SrcPtr : PtrClean;
    type DstPtr;
    type DstBlob : Blob;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>;

    fn encode_blob(&self) -> Self::DstBlob;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        let blob = self.encode_blob();
        Blob::encode_bytes(&blob, dst)
    }
}

pub trait SaveRef<DstPtr> : LoadRef {
    type DstBlob : ?Sized + BlobDyn<Metadata = Self::Metadata>;

    type SaveRefPoll : SaveRefPoll<SrcPtr = <Self::Ptr as Ptr>::Clean, DstPtr = DstPtr, DstBlob = Self::DstBlob>;

    fn init_save_ref(&self) -> Self::SaveRefPoll;

    fn init_save_ref_from_bytes(bytes: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<Self::SaveRefPoll, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let this = Self::load_ref_from_bytes(bytes, zone)?.trust();
        Ok(this.init_save_ref())
    }
}

pub trait SaveRefPoll {
    type SrcPtr : PtrClean;
    type DstPtr;
    type DstBlob : ?Sized + BlobDyn;

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>;

    fn blob_metadata(&self) -> <Self::DstBlob as Pointee>::Metadata;
    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob>;
}

impl<Q, T: Save<Q>> SaveRef<Q> for T {
    type DstBlob = T::DstBlob;
    type SaveRefPoll = T::SavePoll;

    fn init_save_ref(&self) -> Self::SaveRefPoll {
        self.init_save()
    }

    fn init_save_ref_from_bytes(bytes: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<Self::SaveRefPoll, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        Self::init_save_from_bytes(bytes, zone)
    }
}

impl<T: SavePoll> SaveRefPoll for T {
    type SrcPtr = T::SrcPtr;
    type DstPtr = T::DstPtr;
    type DstBlob = T::DstBlob;

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        self.save_poll(saver)
    }

    fn blob_metadata(&self) -> () {
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        self.encode_blob_bytes(dst)
    }
}
