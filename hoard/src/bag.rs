use std::ptr::null;
use std::task::Poll;
use std::fmt;
use std::any;
use std::mem::ManuallyDrop;

use thiserror::Error;

use crate::blob::*;
use crate::load::{Load, LoadRef, MaybeValid};
use crate::save::*;
use crate::owned::{Ref, Take, IntoOwned, Own};

use crate::zone::{AsPtr, FromPtr, Ptr, PtrConst, PtrBlob, Zone, AsZone, Get, GetMut};
use crate::pointee::Pointee;
use crate::primitive::Primitive;

pub struct Bag<T: ?Sized + Pointee, Z, P: Ptr> {
    ptr: P,
    metadata: *const T,
    zone: Z,
}

impl<T: ?Sized + Pointee, Z, P: Ptr> Drop for Bag<T, Z, P> {
    fn drop(&mut self) {
        let metadata = T::metadata(self.metadata);

        unsafe {
            self.ptr.dealloc::<T>(metadata);
        }
    }
}

impl<T: ?Sized + Pointee, Z, P: Ptr> Bag<T, Z, P> {
    pub fn new(src: impl Take<T>) -> Self
        where Z: Default, P: Default,
    {
        let (ptr, metadata, ()) = P::alloc(src).into_raw_parts();

        unsafe {
            Self::from_raw_parts(ptr, metadata, Z::default())
        }
    }

    pub unsafe fn from_raw_parts(ptr: P, metadata: T::Metadata, zone: Z) -> Self {
        Self {
            ptr,
            metadata: T::make_fat_ptr(null(), metadata),
            zone,
        }
    }

    pub fn into_raw_parts(self) -> (P, T::Metadata, Z) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (std::ptr::read(&this.ptr),
             T::metadata(this.metadata),
             std::ptr::read(&this.zone))
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        T::metadata(self.metadata)
    }

    pub fn try_get_dirty(&self) -> Result<&T, P::Clean> {
        unsafe {
            self.ptr.try_get_dirty::<T>(self.metadata())
        }
    }

    pub fn try_get_dirty_mut(&mut self) -> Result<&mut T, P::Clean> {
        unsafe {
            self.ptr.try_get_dirty_mut::<T>(self.metadata())
        }
    }

    pub fn try_take_dirty(self) -> Result<T::Owned, P::Clean>
        where T: IntoOwned
    {
        let (ptr, metadata, _zone) = self.into_raw_parts();

        unsafe {
            ptr.try_take_dirty::<T>(metadata)
        }
    }
}

impl<T: ?Sized + Pointee, Z, P: Ptr> Bag<T, Z, P>
where T: LoadRef,
      Z: Zone + AsZone<T::Zone>
{
    pub fn get<'a>(&'a self) -> Result<Ref<'a, T>, Z::Error>
        where Z: Get<P>
    {
        unsafe {
            self.zone.get_unchecked(&self.ptr, self.metadata())
                .map(MaybeValid::trust)
        }
    }

    pub fn take(self) -> Result<T::Owned, Z::Error>
        where Z: Get<P>
    {
        let (ptr, metadata, zone) = self.into_raw_parts();
        unsafe {
            zone.take_unchecked::<T>(ptr, metadata)
                .map(MaybeValid::trust)
        }
    }

    pub fn get_mut<'a>(&'a mut self) -> Result<&mut T, Z::Error>
        where Z: GetMut<P>
    {
        let metadata = self.metadata();
        unsafe {
            self.zone.get_unchecked_mut(&mut self.ptr, metadata)
                .map(MaybeValid::trust)
        }
    }
}

impl<T: ?Sized + Pointee, Z, P: Ptr> fmt::Debug for Bag<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct(any::type_name::<Self>())
            .field("ptr", &self.ptr)
            .field("metadata", &self.metadata())
            .field("zone", &self.zone)
            .finish()
    }
}

#[derive(Error)]
#[error("FIXME")]
pub enum DecodeBagBytesError<T: ?Sized + BlobDyn, Z: Blob, P: PtrBlob> {
    Ptr(P::DecodeBytesError),
    Metadata(<T::Metadata as Primitive>::DecodeBytesError),
    Layout(<T as Pointee>::LayoutError),
    Zone(Z::DecodeBytesError),
}

impl<T: ?Sized + BlobDyn, Z: Blob, P: PtrBlob> fmt::Debug for DecodeBagBytesError<T, Z, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
}

impl<T: ?Sized + Pointee, Z: Blob, P: Ptr> Blob for Bag<T, Z, P>
where T: BlobDyn,
      P: PtrBlob,
{
    const SIZE: usize = <P as Blob>::SIZE + <Z as Blob>::SIZE + <T::Metadata as Blob>::SIZE;
    type DecodeBytesError = DecodeBagBytesError<T, Z, P>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.ptr)
           .write_field(&self.metadata())
           .write_field(&self.zone)
           .done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();

        let ptr = fields.trust_field().map_err(DecodeBagBytesError::Ptr)?;
        let metadata = fields.trust_field().map_err(DecodeBagBytesError::Metadata)?;
        let zone = fields.trust_field().map_err(DecodeBagBytesError::Zone)?;
        T::try_size(metadata).map_err(DecodeBagBytesError::Layout)?;
        fields.assert_done();

        unsafe {
            Ok(Self::from_raw_parts(ptr, metadata, zone).into())
        }
    }
}

/*
#[derive(Error)]
#[error("FIXME")]
pub enum ValidateBagError<T: ?Sized + BlobDyn, P: PtrBlob> {
    Ptr {
        ptr: P,
        metadata: T::Metadata,
        err: Box<dyn std::error::Error>,
    },
    PointeeBytes(T::DecodeBytesError),
    PointeeChildren(T::ValidateError),
}

impl<T: ?Sized + BlobDyn, P: PtrBlob> fmt::Debug for ValidateBagError<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
}

pub struct BagValidator<T: ?Sized + BlobDyn, P: Ptr> {
    ptr: P,
    metadata: T::Metadata,
    state: State<T::ValidatePoll>,
}

enum State<T> {
    Pending,
    Polling(T),
    Done,
}

impl<T: ?Sized + Pointee, P: PtrBlob> ValidatePoll for BagValidator<T, P>
where T: BlobDyn,
      P: FromPtr<T::Ptr>,
{
    type Ptr = P;
    type Error = ValidateBagError<T, P>;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = P>
    {
        loop {
            self.state = match &mut self.state {
                State::Pending => {
                    let r = validator.check_blob(self.ptr, self.metadata, |maybe_blob| {
                        maybe_blob.map(T::validate_bytes_children)
                    }).map_err(|ptr_err| ValidateBagError::Ptr {
                        ptr: self.ptr,
                        metadata: self.metadata,
                        err: Box::new(ptr_err),
                    })?;

                    match r {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Some(Err(decode_err))) => {
                            return Err(ValidateBagError::PointeeBytes(decode_err)).into();
                        },
                        Poll::Ready(Some(Ok(pointee_poll))) => State::Polling(pointee_poll),
                        Poll::Ready(None) => State::Done,
                    }
                },
                State::Polling(pointee_poll) => {
                    match pointee_poll.validate_poll(validator)
                                      .map_err(ValidateBagError::PointeeChildren)?
                    {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => State::Done,
                    }
                }
                State::Done => break Ok(()).into(),
            };
        }
    }
}
*/

impl<T: ?Sized + Pointee, Z, P: Ptr> Load for Bag<T, Z, P>
where T: LoadRef, Z: Zone,
{
    type Blob = Bag<T::BlobDyn, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let ptr = P::from_clean(P::Clean::from_blob(blob.ptr));
        let metadata = blob.metadata();
        unsafe {
            Self::from_raw_parts(ptr, metadata, *zone)
        }
    }
}

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
