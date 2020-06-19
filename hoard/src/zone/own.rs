use std::any::{self, Any};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::ptr;

use thiserror::Error;

use owned::Take;

use super::*;

use crate::refs::Ref;
use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::*;
use crate::save::*;
//use crate::primitive::Primitive;

#[repr(C)]
pub struct Own<T: ?Sized + Pointee, P: Ptr, M: 'static = <T as Pointee>::Metadata> {
    marker: PhantomData<T>,
    inner: Fat<T, P, M>
}

impl<T: ?Sized + Pointee, P: Ptr, M> AsRef<Fat<T, P, M>> for Own<T, P, M> {
    fn as_ref(&self) -> &Fat<T, P, M> {
        &self.inner
    }
}

unsafe impl<#[may_dangle] T: ?Sized + Pointee, P: Ptr, M> Drop for Own<T, P, M> {
    fn drop(&mut self) {
        unsafe {
            let metadata: &dyn Any = &self.metadata;
            let metadata: &T::Metadata = metadata.downcast_ref().unwrap();
            self.raw.dealloc::<T>(*metadata)
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M> Deref for Own<T, P, M> {
    type Target = Fat<T, P, M>;

    fn deref(&self) -> &Self::Target {
        &self.as_ref()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M> fmt::Debug for Own<T, P, M>
where P: fmt::Debug,
      M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(any::type_name::<Self>())
            .field(&self.inner)
            .finish()
    }
}

/*
impl<T: ?Sized + Pointee, P: Ptr> Own<T, P> {
    pub fn get_in<'a, Z: Get<P>>(&'a self, zone: &Z) -> Ref<'a, T>
        where T: Load<P>
    {
        unsafe { zone.get_unchecked::<T>(&self.inner.raw, self.inner.metadata) }
    }

    pub fn try_get_in<'a, Z: TryGet<P>>(&'a self, zone: &Z) -> Result<Ref<'a, T>, Z::Error>
        where T: Load<P>
    {
        unsafe { zone.try_get_unchecked::<T>(&self.inner.raw, self.inner.metadata) }
    }

    pub fn get_mut_in<'a, Z: GetMut<P>>(&'a mut self, zone: &Z) -> &'a mut T
        where T: Load<P>
    {
        unsafe { zone.get_mut_unchecked::<T>(&mut self.inner.raw, self.inner.metadata) }
    }

    pub fn try_get_mut_in<'a, Z: TryGetMut<P>>(&'a mut self, zone: &Z) -> Result<&'a mut T, Z::Error>
        where T: Load<P>
    {
        unsafe { zone.try_get_mut_unchecked::<T>(&mut self.inner.raw, self.inner.metadata) }
    }

    pub fn take_in<'a, Z: Get<P>>(self, zone: &Z) -> T::Owned
        where T: Load<P>
    {
        let fat = self.into_inner();
        unsafe { zone.take_unchecked::<T>(fat.raw, fat.metadata) }
    }

    pub fn try_take_in<'a, Z: TryGet<P>>(self, zone: &Z) -> Result<T::Owned, Z::Error>
        where T: Load<P>
    {
        let fat = self.into_inner();
        unsafe { zone.try_take_unchecked::<T>(fat.raw, fat.metadata) }
    }

    pub fn try_get_dirty<'a>(&'a self) -> Result<&'a T, P::Persist> {
        unsafe {
            self.inner.raw.try_get_dirty_unchecked::<T>(self.inner.metadata)
        }
    }
}
*/

impl<T: ?Sized + Pointee, P: Ptr, M> Own<T, P, M> {
    pub unsafe fn new_unchecked(inner: Fat<T, P, M>) -> Self {
        Self { marker: PhantomData, inner, }
    }

    pub fn into_inner(self) -> Fat<T, P, M> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&this.inner) }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> BlobSize for Own<T, P> {
    const BLOB_LAYOUT: BlobLayout = <P::Persist as BlobSize>::BLOB_LAYOUT.extend(T::Metadata::BLOB_LAYOUT);
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateOwnBlobError<P: fmt::Debug, M: fmt::Debug> {
    Ptr(P),
    Metadata(M),
}

impl<V: Copy, T: ?Sized + Pointee, P: Ptr> ValidateBlob<V> for Own<T, P>
where P: BlobSize + ValidateBlob<V>,
{
    type Error = ValidateOwnBlobError<P::Error, <T::Metadata as ValidateBlob<V>>::Error>;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut fields = blob.validate_fields(padval);
        fields.field::<P>().map_err(ValidateOwnBlobError::Ptr)?;
        fields.field::<T::Metadata>().map_err(ValidateOwnBlobError::Metadata)?;
        unsafe { Ok(fields.finish()) }
    }
}

impl<Z, T: ?Sized + Pointee, P: Ptr> Load<Z> for Own<T, P>
where T::Metadata: Decode<Z>,
      P: Decode<Z>,
{
    fn decode_blob(blob: ValidBlob<Self>, zone: &Z) -> Self
        where Z: BlobZone
    {
        let mut fields = blob.decode_fields(zone);
        let r = unsafe {
            Self {
                marker: PhantomData,
                inner: Fat {
                    _marker: PhantomData,
                    raw: fields.decode_unchecked(),
                    metadata: fields.decode_unchecked(),
                },
            }
        };
        fields.finish();
        r
    }
}

impl<Z, T: ?Sized + Pointee, P: Ptr> Decode<Z> for Own<T, P>
where P: Decode<Z>,
{}

unsafe impl<T: ?Sized + Pointee, P: Ptr, M> Persist for Own<T, P, M>
where P: Persist,
      M: Persist,
{}

pub struct OwnSavePoll<Y: Zone, Q: Ptr, T: ?Sized + Save<Y, Q>> {
    state: State<Y, Q, T>,
    metadata: T::Metadata,
}

enum State<Y: Zone, Q: Ptr, T: ?Sized + Save<Y, Q>> {
    Clean(Q::Persist),
    Dirty(T::SavePoll),
    Done(Y::PersistPtr),
}

impl<Y: Zone, T: ?Sized + Saved<Y>, P: Ptr> Saved<Y> for Own<T, P> {
    type Saved = Own<T::Saved, Y::Ptr>;
}

impl<Y: Zone, Q: Ptr, T: ?Sized + Save<Y, Q>, P: Ptr> Save<Y, Q> for Own<T, P>
where P: AsPtr<Q>
{
    type SavePoll = OwnSavePoll<Y, Q, T>;

    fn init_save(&self) -> Self::SavePoll {
        match unsafe { self.raw.as_ptr().try_get_dirty_unchecked::<T>(self.metadata) } {
            Ok(dirty) => {
                OwnSavePoll {
                    metadata: self.metadata,
                    state: State::Dirty(dirty.init_save()),
                }
            },
            Err(persist_ptr) => {
                OwnSavePoll {
                    metadata: self.metadata,
                    state: State::Clean(persist_ptr),
                }
            },
        }
    }
}

impl<Y: Zone, Q: Ptr, T: ?Sized + Save<Y, Q>> SavePoll<Y, Q> for OwnSavePoll<Y, Q, T> {
    type Target = Own<T::Saved, Y::Ptr>;

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>
    {
        loop {
            self.state = match &mut self.state {
                State::Clean(persist_ptr) => {
                    let ptr: &Q = persist_ptr.as_ptr();
                    match unsafe { dst.try_save_ptr::<T>(ptr, self.metadata)? } {
                        Ok(dst_ptr) => State::Done(dst_ptr),
                        Err(value) => State::Dirty(value.init_save()),
                    }
                },
                State::Dirty(value_poll) => {
                    value_poll.save_children(dst)?;

                    // Note how if this fails save_children() will be called twice.
                    State::Done(dst.save(value_poll)?)
                },
                State::Done(_) => break Ok(())
            };
        }
    }

    fn save_blob<W: WriteBlob<Y, Q>>(&self, dst: W) -> Result<W::Done, W::Error>
        where Y: BlobZone
    {
        todo!()
    }
}

/*
impl<Y: BlobZone, Q, T: ?Sized + Pointee, P: Ptr> EncodeBlob<Y, Q> for Own<T, P>
where
      T: SaveBlob<Y, Q>,
      T::Metadata: Decode<Y>,
      P: AsPtr<Q>,
{
    type Encoded = Own<T::Saved, Y::Ptr>;
    type EncodeBlobPoll = OwnEncodeBlobPoll<Y::BlobPtr, T::SaveBlobPoll, T::Metadata>;

    fn init_encode_blob<D>(&self, dst: &D) -> Result<Self::EncodeBlobPoll, D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = Q>
    {
        Ok(OwnEncodeBlobPoll {
            metadata: self.metadata,
            state: match unsafe { dst.try_get_dirty::<T>(&self.raw.as_ptr(), self.metadata)? } {
                Ok(value) => State::Poll(value.init_save_blob(dst)?),
                Err(ptr) => State::Done(ptr),
            },
        })
    }
}

impl<Y: BlobZone, T, Q> EncodeBlobPoll<Y, Q> for OwnEncodeBlobPoll<Y::BlobPtr, T, <T::Target as Pointee>::Metadata>
where T: SaveBlobPoll<Y, Q>,
      <T::Target as Pointee>::Metadata: Decode<Y>,
{
    type Target = Own<T::Target, Y::Ptr>;

    fn encode_blob_poll<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = Q>
    {
        loop {
            self.state = match &mut self.state {
                State::Poll(value) => {
                    /*
                    dst = value.save_poll(dst)?;

                    let (d, r_ptr) = dst.try_save_ptr(value)?;
                    dst = d;
                    State::Done(r_ptr)
                    */ todo!()
                },
                State::Done(_) => break Ok(()),
            }
        }
    }

    fn encode_blob<W: Write>(&self, dst: &mut W) -> Result<(), W::Error> {
        todo!()
    }
    /*
    fn encode_poll<D>(&mut self, mut dst: &mut D) -> Result<(), D::Error>
        where D: blob::Saver<Y, P>
    {
        /*
        */ todo!()
    }

    fn encode<W: blob::Write>(&self, dst: &mut W) -> Result<(), W::Error> {
        todo!()
    }
    */
}
*/

/*
impl<T, M, R> EncodeBlob for OwnEncodePoll<T, M, R>
where R: Primitive,
      M: Primitive,
{
    const BLOB_LEN: usize = R::BLOB_LEN + M::BLOB_LEN;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        if let State::Done(r_ptr) = &self.state {
            dst.write_primitive(r_ptr)?
               .write_primitive(&self.metadata)?
               .done()
        } else {
            panic!()
        }
    }
}
*/
