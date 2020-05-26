use std::any::{self, Any};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::ptr;

use thiserror::Error;

use owned::Take;

use super::*;

use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::primitive::Primitive;

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

impl<T: ?Sized + Pointee, P: Ptr, M> Drop for Own<T, P, M> {
    fn drop(&mut self) {
        unsafe {
            let metadata: &dyn Any = &self.metadata;
            let metadata = metadata.downcast_ref().unwrap();
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
}

impl<T: ?Sized + Pointee, P: Ptr, M> Own<T, P, M> {
    pub unsafe fn new_unchecked(inner: Fat<T, P, M>) -> Self {
        Self { marker: PhantomData, inner, }
    }

    pub fn into_inner(self) -> Fat<T, P, M> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&this.inner) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateOwnBlobError<P: fmt::Debug, M: fmt::Debug> {
    Ptr(P),
    Metadata(M),
}

impl<T: ?Sized + Pointee, P: Ptr, M> ValidateBlob for Own<T, P, M>
where P: ValidateBlob,
      M: ValidateBlob,
{
    type Error = ValidateOwnBlobError<P::Error, M::Error>;

    const BLOB_LEN: usize = P::BLOB_LEN + M::BLOB_LEN;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<P>().map_err(ValidateOwnBlobError::Ptr)?;
        blob.field::<M>().map_err(ValidateOwnBlobError::Metadata)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Q: Ptr, T: ?Sized + Pointee, P: Ptr> Decode<Q> for Own<T, P>
where T::Metadata: Decode<Q>,
      P: Decode<Q>,
{
    fn decode_blob(mut blob: BlobDecoder<Q, Self>) -> Self {
        let r = unsafe {
            Self {
                marker: PhantomData,
                inner: Fat {
                    _marker: PhantomData,
                    raw: blob.field_unchecked(),
                    metadata: blob.field_unchecked(),
                },
            }
        };
        blob.finish();
        r
    }
}

unsafe impl<T: ?Sized + Pointee, P: Ptr, M> Persist for Own<T, P, M>
where P: Persist,
      M: Persist,
{}

#[derive(Debug)]
pub struct OwnEncoder<T, M, R> {
    state: State<T, R>,
    metadata: M,
}

#[derive(Debug)]
enum State<T, R> {
    Poll(T),
    Done(R),
}

impl<Q, R, T: ?Sized + Pointee, P: Ptr> Encode<Q, R> for Own<T, P>
where R: Primitive,
      T: Save<Q, R>,
      T::Metadata: Primitive,
      P: AsPtr<Q>,
{
    type EncodePoll = OwnEncoder<T::SavePoll, T::Metadata, R>;

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
        OwnEncoder {
            metadata: self.metadata,
            state: match unsafe { dst.check_dirty::<T>(&self.raw.as_ptr(), self.metadata) } {
                Ok(r_ptr) => State::Done(r_ptr),
                Err(value) => State::Poll(value.init_save(dst)),
            },
        }
    }
}

impl<Q, T, M, R> SavePoll<Q, R> for OwnEncoder<T, M, R>
where T: SavePoll<Q, R> + SaveBlob
{
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, mut dst: D) -> Result<D, D::Error> {
        loop {
            self.state = match &mut self.state {
                State::Poll(value) => {
                    dst = value.save_poll(dst)?;

                    let (d, r_ptr) = dst.try_save_ptr(value)?;
                    dst = d;
                    State::Done(r_ptr)
                },
                State::Done(_) => break Ok(dst),
            }
        }
    }
}

impl<T, M, R> EncodeBlob for OwnEncoder<T, M, R>
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
