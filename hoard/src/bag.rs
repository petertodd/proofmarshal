use std::marker::PhantomData;
use std::fmt;
use std::any;
use std::mem::ManuallyDrop;

use thiserror::Error;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::{Load, LoadRefIn, MaybeValid};
use crate::owned::{Ref, Take, IntoOwned, RefOwn};
use crate::ptr::{Ptr, PtrClean, PtrBlob, Get, TryGet, GetMut, TryGetMut};

#[repr(C)]
pub struct Bag<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<T>,
    ptr: P,
    metadata: T::Metadata,
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for Bag<T, P> {
    fn drop(&mut self) {
        unsafe {
            self.ptr.dealloc::<T>(self.metadata);
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Bag<T, P> {
    /*
    pub fn new(src: impl Take<T>) -> Self
        where Z: Default, P: Default,
    {
        let (ptr, metadata, ()) = P::alloc(src).into_raw_parts();

        unsafe {
            Self::from_raw_parts(ptr, metadata, Z::default())
        }
    }
    */

    pub unsafe fn from_raw_parts(ptr: P, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr,
            metadata,
        }
    }

    pub fn into_raw_parts(self) -> (P, T::Metadata) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (std::ptr::read(&this.ptr),
             this.metadata)
        }
    }

    pub fn ptr(&self) -> &P {
        &self.ptr
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }

    pub fn try_get_dirty(&self) -> Result<&T, P::Clean> {
        unsafe {
            self.ptr.try_get_dirty::<T>(self.metadata())
        }.map(|r| r.trust())
    }

    pub fn try_get_dirty_mut(&mut self) -> Result<&mut T, P::Clean> {
        unsafe {
            self.ptr.try_get_dirty_mut::<T>(self.metadata())
        }.map(|r| r.trust())
    }

    pub fn try_take_dirty(self) -> Result<T::Owned, P::Clean>
        where T: IntoOwned
    {
        let (ptr, metadata) = self.into_raw_parts();

        unsafe {
            ptr.try_take_dirty::<T>(metadata)
        }.map(|r| r.trust())
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Bag<T, P>
where T: LoadRefIn<P::Zone>,
{
    #[track_caller]
    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where P: Get
    {
        unsafe {
            self.ptr.get(self.metadata())
        }.trust()
    }

    pub fn try_get<'a>(&'a self) -> Result<Ref<'a, T>, P::Error>
        where P: TryGet
    {
        unsafe {
            self.ptr.try_get(self.metadata())
                    .map(MaybeValid::trust)
        }
    }

    pub fn get_mut<'a>(&'a mut self) -> &'a mut T
        where P: GetMut
    {
        unsafe {
            self.ptr.get_mut(self.metadata())
        }.trust()
    }

    pub fn try_get_mut<'a>(&'a mut self) -> Result<&'a mut T, P::Error>
        where P: TryGetMut
    {
        unsafe {
            self.ptr.try_get_mut(self.metadata())
                    .map(MaybeValid::trust)
        }
    }

/*
    pub fn take(self) -> T::Owned
        where Z: Get<P>
    {
        let (ptr, metadata, zone) = self.into_raw_parts();
        unsafe {
            zone.take_unchecked::<T>(ptr, metadata)
        }.trust()
    }

    pub fn try_take(self) -> Result<T::Owned, Z::Error>
        where Z: TryGet<P>
    {
        let (ptr, metadata, zone) = self.into_raw_parts();
        unsafe {
            zone.try_take_unchecked::<T>(ptr, metadata)
                .map(MaybeValid::trust)
        }
    }

    pub fn get_mut<'a>(&'a mut self) -> &mut T
        where Z: GetMut<P>
    {
        let metadata = self.metadata();
        unsafe {
            self.zone.get_unchecked_mut(&mut self.ptr, metadata)
        }.trust()
    }

    pub fn try_get_mut<'a>(&'a mut self) -> Result<&mut T, Z::Error>
        where Z: TryGetMut<P>
    {
        let metadata = self.metadata();
        unsafe {
            self.zone.try_get_unchecked_mut(&mut self.ptr, metadata)
                     .map(MaybeValid::trust)
        }
    }
*/
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for Bag<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct(any::type_name::<Self>())
            .field("ptr", &self.try_get_dirty().map_err(P::from_clean))
            .field("metadata", &self.metadata)
            .finish()
    }
}

#[derive(Error)]
#[error("FIXME")]
pub enum DecodeBagBytesError<T: ?Sized + BlobDyn, P: PtrBlob> {
    Ptr(P::DecodeBytesError),
    Metadata(<T::Metadata as Blob>::DecodeBytesError),
    Layout(<T as Pointee>::LayoutError),
}

impl<T: ?Sized + BlobDyn, P: PtrBlob> fmt::Debug for DecodeBagBytesError<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Blob for Bag<T, P>
where T: BlobDyn,
      P: PtrBlob,
{
    const SIZE: usize = <P as Blob>::SIZE + <T::Metadata as Blob>::SIZE;
    type DecodeBytesError = DecodeBagBytesError<T, P>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.ptr)
           .write_field(&self.metadata())
           .done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();

        let ptr = fields.trust_field().map_err(DecodeBagBytesError::Ptr)?;
        let metadata = fields.trust_field().map_err(DecodeBagBytesError::Metadata)?;
        T::try_size(metadata).map_err(DecodeBagBytesError::Layout)?;
        fields.assert_done();

        unsafe {
            Ok(Self::from_raw_parts(ptr, metadata).into())
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Load for Bag<T, P>
where T: LoadRefIn<P::Zone>
{
    type Blob = Bag<T::BlobDyn, P::Blob>;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let ptr = P::from_clean(P::Clean::from_blob(blob.ptr, zone));
        let metadata = blob.metadata();
        unsafe {
            Self::from_raw_parts(ptr, metadata)
        }
    }
}

/*
impl<Y: Zone, Q: Ptr, T: ?Sized, Z, P: Ptr> Saved<Y, Q> for Bag<T, Z, P>
where T: SavedRef<Y, Q>,
{
    type Saved = Bag<T::SavedRef, Y, Q>;
}

impl<T: ?Sized + SaveDirtyRef, Z: Zone, P: Ptr> SaveDirty for Bag<T, Z, P>
where T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveDirtyPoll = BagSaveDirtyPoll<T::SaveDirtyRefPoll, P::Clean>;

    fn init_save_dirty(&self) -> Self::SaveDirtyPoll {
        BagSaveDirtyPoll {
            metadata: self.metadata(),
            state: match self.try_get_dirty() {
                Ok(dirty) => State::Dirty(dirty.init_save_dirty_ref()),
                Err(p_clean) => {
                    State::Done(p_clean.to_blob())
                }
            }
        }
    }
}

pub struct BagSaveDirtyPoll<T: SaveDirtyRefPoll, P: PtrConst> {
    metadata: <T::SavedBlobDyn as Pointee>::Metadata,
    state: State<T, P::Blob>,
}

enum State<T, P> {
    Dirty(T),
    Done(P),
}

impl<T: SaveDirtyRefPoll, P: PtrConst> SaveDirtyPoll for BagSaveDirtyPoll<T, P>
where T::CleanPtr: FromPtr<P>
{
    type CleanPtr = P;
    type SavedBlob = Bag<T::SavedBlobDyn, (), P::Blob>;

    fn save_dirty_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>
    {
        loop {
            self.state = match &mut self.state {
                State::Dirty(dirty) => {
                    dirty.save_dirty_ref_poll(saver)?;

                    let q_blob = saver.save_bytes(self.metadata, |dst| {
                        dirty.encode_blob_dyn_bytes(dst)
                    })?;

                    State::Done(q_blob)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        match self.state {
            State::Done(ptr_blob) => {
                unsafe { Bag::from_raw_parts(ptr_blob, self.metadata, ()) }
            },
            State::Dirty(_) => panic!(),
        }
    }
}

/*
impl<Y: Zone, Q: Ptr, T: ?Sized + Pointee, Z: Zone, P: Ptr> Save<Y, Q> for Bag<T, Z, P>
where T: SaveRef<Y, Q>,
      Q::Blob: PtrConst,
      P::Blob: PtrConst + Into<Q::Blob>,
{
    type SavePoll = BagSavePoll<Q, T::SaveRefPoll>;

    fn init_save(&self) -> Self::SavePoll {
        BagSavePoll {
            metadata: self.metadata(),
            state: match self.try_get_dirty() {
                Ok(dirty) => State::Dirty(dirty.init_save_ref()),
                Err(p_clean) => {
                    let p_blob: P::Blob = p_clean.to_blob();
                    let q_blob: Q::Blob = p_blob.into();
                    State::Done(q_blob)
                }
            }
        }
    }
}

pub struct BagSavePoll<Q: Ptr, T: SaveRefPoll<Q>> {
    metadata: <T::SavedBlob as Pointee>::Metadata,
    state: State<Q, T>,
}

enum State<Q: Ptr, T> {
    Dirty(T),
    Done(<Q as Load>::Blob),
}

impl<Q: Ptr, T> SavePoll<Q> for BagSavePoll<Q, T>
where T: SaveRefPoll<Q>,
      Q::Blob: PtrConst,
{
    type SavedBlob = Bag<T::SavedBlob, (), Q::Blob>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<DstPtr = Q>
    {
        loop {
            self.state = match &mut self.state {
                State::Dirty(dirty) => {
                    dirty.save_ref_poll(saver)?;

                    let q_blob = saver.save_bytes(self.metadata, |dst| {
                        dirty.encode_blob_bytes(dst)
                    })?;

                    State::Done(q_blob)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        match self.state {
            State::Done(q_blob) => {
                unsafe { Bag::from_raw_parts(q_blob, self.metadata, ()) }
            },
            State::Dirty(_) => panic!(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
*/
*/
