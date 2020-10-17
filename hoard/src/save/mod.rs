use std::marker::PhantomData;

use crate::load::*;
use crate::blob::*;
use crate::zone::{Ptr, PtrConst, Zone, FromPtr};
use crate::pointee::Pointee;

pub mod impls;

pub trait Saved<Y, Q> {
    type Saved : Load;
}

pub trait SavedRef<Y, Q> : Pointee {
    type SavedRef : ?Sized + LoadRef + Pointee<Metadata=Self::Metadata>;
}

impl<Y, Q, T: Saved<Y, Q>> SavedRef<Y, Q> for T {
    type SavedRef = T::Saved;
}

pub trait BlobSaver {
    type Error;
    type CleanPtr : PtrConst;

    fn save_bytes<T: ?Sized + BlobDyn>(
        &mut self,
        metadata: T::Metadata,
        f: impl FnOnce(BytesUninit<T>) -> Bytes<T>,
    ) -> Result<<Self::CleanPtr as PtrConst>::Blob, Self::Error>;

    fn save_blob<T: Blob>(&mut self, blob: &T) -> Result<<Self::CleanPtr as PtrConst>::Blob, Self::Error> {
        self.save_bytes((), |dst| {
            blob.encode_bytes(dst)
        })
    }
}

#[repr(transparent)]
struct SaverAdapter<S, Q> {
    marker: PhantomData<fn() -> Q>,
    inner: S,
}

impl<S, Q> SaverAdapter<S, Q> {
    fn new(inner: S) -> Self {
        Self {
            marker: PhantomData,
            inner,
        }
    }

    fn from_mut(inner: &mut S) -> &mut Self {
        // SAFETY: #[repr(transparent)]
        unsafe {
            &mut *(inner as *mut S as *mut Self)
        }
    }
}

impl<S: BlobSaver, Q: PtrConst> BlobSaver for SaverAdapter<S, Q>
where Q: FromPtr<S::CleanPtr>
{
    type Error = S::Error;
    type CleanPtr = Q;

    fn save_bytes<T: ?Sized + BlobDyn>(
        &mut self,
        metadata: T::Metadata,
        f: impl FnOnce(BytesUninit<T>) -> Bytes<T>,
    ) -> Result<Q::Blob, Self::Error>
    {
        self.inner.save_bytes(metadata, f)
                  .map(|s_blob| {
                      let s_ptr = S::CleanPtr::from_blob(s_blob);
                      let q_ptr = Q::from_ptr(s_ptr);
                      q_ptr.to_blob()
                  })
    }
}


pub trait SaveDirty : Load {
    type CleanPtr : PtrConst;
    type SaveDirtyPoll : SaveDirtyPoll<CleanPtr = Self::CleanPtr, SavedBlob = Self::Blob>;

    fn init_save_dirty(&self) -> Self::SaveDirtyPoll;
}

pub trait SaveDirtyPoll {
    type CleanPtr : PtrConst;
    type SavedBlob : Blob;

    fn save_dirty_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>;

    fn save_dirty_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver,
              Self::CleanPtr: FromPtr<S::CleanPtr>
    {
        self.save_dirty_poll_impl(SaverAdapter::from_mut(saver))
    }

    fn encode_blob(&self) -> Self::SavedBlob;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlob>) -> Bytes<'a, Self::SavedBlob> {
        let blob = self.encode_blob();
        Blob::encode_bytes(&blob, dst)
    }
}

pub trait SaveDirtyRef : LoadRef {
    type CleanPtr : PtrConst;
    type SaveDirtyRefPoll : SaveDirtyRefPoll<CleanPtr = Self::CleanPtr, SavedBlobDyn = Self::BlobDyn>;

    fn init_save_dirty_ref(&self) -> Self::SaveDirtyRefPoll;
}

impl<T: SaveDirty> SaveDirtyRef for T {
    type CleanPtr = T::CleanPtr;
    type SaveDirtyRefPoll = T::SaveDirtyPoll;

    fn init_save_dirty_ref(&self) -> Self::SaveDirtyRefPoll {
        self.init_save_dirty()
    }
}

pub trait SaveDirtyRefPoll {
    type CleanPtr : PtrConst;
    type SavedBlobDyn : ?Sized + BlobDyn;

    fn blob_metadata(&self) -> <Self::SavedBlobDyn as Pointee>::Metadata;

    fn save_dirty_ref_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>;

    fn save_dirty_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver,
              Self::CleanPtr: FromPtr<S::CleanPtr>,
    {
        self.save_dirty_ref_poll_impl(SaverAdapter::from_mut(saver))
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlobDyn>) -> Bytes<'a, Self::SavedBlobDyn>;
}

impl<T: SaveDirtyPoll> SaveDirtyRefPoll for T {
    type CleanPtr = T::CleanPtr;
    type SavedBlobDyn = T::SavedBlob;

    fn blob_metadata(&self) -> <Self::SavedBlobDyn as Pointee>::Metadata {
        ()
    }

    fn save_dirty_ref_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>
    {
        self.save_dirty_poll_impl(saver)
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlobDyn>) -> Bytes<'a, Self::SavedBlobDyn> {
        self.encode_blob_bytes(dst)
    }
}

/*
pub trait SaveRef<DstPtr: Ptr> : LoadRef {
    type SaveRefPoll : SaveRefPoll<DstPtr, Self::CleanPtr>;

    fn init_save_ref(&self) -> Self::SaveRefPoll;
}

pub trait SaveRefPoll<Q: Ptr, CleanPtr: PtrConst> {
    type SavedBlob : BlobDyn;


    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<DstPtr = Q>;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlob>) -> Bytes<'a, Self::SavedBlob>;
}
*/

/*
impl<Y, Q: Ptr, T: Save<Y, Q>> SaveRef<Y, Q> for T {
    type SaveRefPoll = T::SavePoll;

    fn init_save_ref(&self) -> Self::SaveRefPoll {
        self.init_save()
    }
}


impl<Q: Ptr, T: SavePoll<Q>> SaveRefPoll<Q> for T {
    type SavedBlob = T::SavedBlob;

    fn blob_metadata(&self) -> <Self::SavedBlob as Pointee>::Metadata {
        ()
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<DstPtr = Q>
    {
        self.save_poll(saver)
    }

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlob>) -> Bytes<'a, Self::SavedBlob> {
        self.encode_blob_bytes(dst)
    }
}

*/
