//! A `Box` equivalent for data behind zone pointers.

use std::marker::PhantomData;
use std::fmt;
use std::any;
use std::mem::ManuallyDrop;

use thiserror::Error;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::{Load, LoadRef, MaybeValid};
use crate::owned::{Ref, Take, IntoOwned, RefOwn};
use crate::ptr::{Ptr, PtrClean, PtrBlob, Get, TryGet, GetMut, TryGetMut, AsZone};
use crate::save::{Save, SavePoll, SaveRef, SaveRefPoll, Saver};

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
where T: LoadRef,
      P::Zone: AsZone<T::Zone>,
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
where T: LoadRef,
{
    type Blob = Bag<T::BlobDyn, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let ptr = P::from_clean(P::Clean::from_blob(blob.ptr, zone));
        let metadata = blob.metadata();
        unsafe {
            Self::from_raw_parts(ptr, metadata)
        }
    }
}

impl<Q: PtrBlob, T: ?Sized + SaveRef<Q>, P: Ptr> Save<Q> for Bag<T, P>
where T: LoadRef,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = Bag<T::DstBlob, Q>;
    type SavePoll = BagSavePoll<Q, T, P>;

    fn init_save(&self) -> Self::SavePoll {
        BagSavePoll {
            metadata: self.metadata(),
            state: match self.try_get_dirty() {
                Ok(dirty) => State::Dirty(dirty.init_save_ref()),
                Err(p_clean) => State::Clean(p_clean),
            }
        }
    }
}

pub struct BagSavePoll<Q, T: ?Sized + SaveRef<Q>, P: Ptr> {
    metadata: T::Metadata,
    state: State<Q, T, P>,
}

enum State<Q, T: ?Sized + SaveRef<Q>, P: Ptr> {
    Clean(P::Clean),
    Dirty(T::SaveRefPoll),
    Done(Q),
}

impl<Q, T: ?Sized + SaveRef<Q>, P: Ptr> fmt::Debug for State<Q, T, P>
where P::Clean: fmt::Debug,
      T::SaveRefPoll: fmt::Debug,
      Q: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Clean(clean) => f.debug_tuple("Clean")
                                    .field(clean)
                                    .finish(),
            State::Dirty(dirty) => f.debug_tuple("Dirty")
                                    .field(dirty)
                                    .finish(),
            State::Done(done) => f.debug_tuple("Done")
                                  .field(done)
                                  .finish(),
        }
    }
}

impl<Q, T: ?Sized + SaveRef<Q>, P: Ptr> fmt::Debug for BagSavePoll<Q, T, P>
where P::Clean: fmt::Debug,
      T::SaveRefPoll: fmt::Debug,
      Q: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BagSavePoll")
            .field("metadata", &self.metadata)
            .field("state", &self.state)
            .finish()
    }
}

impl<Q: PtrBlob, T: ?Sized + SaveRef<Q>, P: Ptr> SavePoll for BagSavePoll<Q, T, P>
where P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = Bag<T::DstBlob, Q>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        loop {
            self.state = match &mut self.state {
                State::Clean(p_clean) => {
                    match saver.save_ptr::<T>(*p_clean, self.metadata)? {
                        Ok(q_ptr) => State::Done(q_ptr),
                        Err(target_poll) => State::Dirty(target_poll),
                    }
                },
                State::Dirty(target) => {
                    State::Done(saver.poll_ref(target)?)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }

    fn encode_blob(&self) -> Self::DstBlob {
        if let State::Done(r_ptr) = &self.state {
            unsafe { Bag::from_raw_parts(*r_ptr, self.metadata) }
        } else {
            panic!()
        }
    }
}
