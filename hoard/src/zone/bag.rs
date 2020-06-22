use std::any::{Any, type_name};
use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ptr;
use std::error::Error;

use thiserror::Error;

use owned::{Take, IntoOwned};

use crate::pointee::Pointee;
use crate::refs::*;
use crate::zone::*;
use crate::blob::*;
use crate::load::*;

#[derive(Debug)]
#[repr(C)]
pub struct Bag<T: ?Sized + Pointee, Z, P: Ptr = <Z as Zone>::Ptr, M: 'static = <T as Pointee>::Metadata> {
    inner: Own<T, P, M>,
    zone: Z,
}

/*
impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z> {
    pub fn new_in(value: impl Take<T>, mut alloc: impl Alloc<Ptr=P, Zone=Z>) -> Self {
        Self {
            inner: alloc.alloc_own(value),
            zone: alloc.zone(),
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z>
where T: Load<P>,
{
    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where Z: Get<P>
    {
        self.inner.get_in(&self.zone)
    }

    pub fn try_get<'a>(&'a self) -> Result<Ref<'a, T>, Z::Error>
        where Z: TryGet<P>
    {
        self.inner.try_get_in(&self.zone)
    }

    pub fn get_mut<'a>(&'a mut self) -> &'a mut T
        where Z: GetMut<P>
    {
        self.inner.get_mut_in(&self.zone)
    }

    pub fn try_get_mut<'a>(&'a mut self) -> Result<&'a mut T, Z::Error>
        where Z: TryGetMut<P>
    {
        self.inner.try_get_mut_in(&self.zone)
    }

    pub fn take(self) -> T::Owned
        where Z: Get<P>
    {
        let (own, zone) = self.into_parts();
        own.take_in(&zone)
    }

    pub fn try_take(self) -> Result<T::Owned, Z::Error>
        where Z: TryGet<P>
    {
        let (own, zone) = self.into_parts();
        own.try_take_in(&zone)
    }
}

impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z> {
    pub fn from_parts(inner: Own<T, P>, zone: Z) -> Self {
        Self { inner, zone }
    }

    pub fn into_parts(self) -> (Own<T, P>, Z) {
        (self.inner, self.zone)
    }
}

impl<T: ?Sized + Pointee, P: Ptr, Z, M> From<Bag<T, P, Z, M>> for Own<T, P, M> {
    fn from(bag: Bag<T, P, Z, M>) -> Self {
        bag.inner
    }
}
*/

impl<T: ?Sized + Pointee, Z, P: Ptr> BlobSize for Bag<T, Z, P> {
    const BLOB_LAYOUT: BlobLayout = <Own<T, P> as BlobSize>::BLOB_LAYOUT;
}

impl<V: Copy, T: ?Sized + Pointee, Z, P: Ptr> ValidateBlob<V> for Bag<T, Z, P> {
    type Error = <Own<T, P> as ValidateBlob<V>>::Error;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

/*
impl<T: ?Sized + Load, Z: Zone, P: Ptr> Decode for Bag<T, Z, P>
where Z: Clone
{
    type Zone = Z;
    type Ptr = P;

    fn decode_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self {
        let mut fields = blob.decode_fields(zone);
        let inner = unsafe { fields.decode_unchecked() };
        fields.finish();

        Self {
            inner,
            zone: zone.clone(),
        }
    }
}
*/

/*
#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateBagBlobError<OwnError: Error, ZoneError: Error> {
    Own(OwnError),
    Zone(ZoneError),
}


impl<V: Copy, T: ?Sized + Pointee, Z, P: Ptr> ValidateBlob<V> for Bag<T, Z, P>
where P: ValidateBlob<V>,
      T::Metadata: ValidateBlob<V>,
      Z: ValidateBlob<V>,
{
    type Error = ValidateBagBlobError<<Own<T, P> as ValidateBlob<V>>::Error, Z::Error>;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut fields = blob.validate_fields(padval);
        fields.field::<Own<T, P>>().map_err(ValidateBagBlobError::Own)?;
        fields.field::<Z>().map_err(ValidateBagBlobError::Zone)?;
        unsafe { Ok(fields.finish()) }
    }
}

impl<Y: Zone, T: ?Sized + Pointee, P: Ptr, Z> Decode<Y> for Bag<T, Z, P>
where Z: Decode<Y>,
      P: Decode<Y>,
      T::Metadata: Decode<Y>,
{
    fn decode_blob(blob: ValidBlob<Self>, zone: &Y) -> Self {
        let mut fields = blob.decode_fields(zone);
        let r = unsafe {
            Self {
                inner: fields.decode_unchecked(),
                zone: fields.decode_unchecked(),
            }
        };
        fields.finish();
        r
    }
}

unsafe impl<T: ?Sized + Pointee, Z, P: Ptr, M> Persist for Bag<T, Z, P, M>
where Z: Persist,
      P: Persist,
      M: Persist,
{}
*/

/*
/*
impl<Q, R, T: ?Sized + Pointee, P: Ptr, Z> Encode<Q, R> for Bag<T, P, Z>
where R: Primitive,
      T: Save<Q, R>,
      Z: Encode<Q, R>,
      P: AsPtr<Q>,
{
    type EncodePoll = (<Own<T, P> as Encode<Q, R>>::EncodePoll, Z::EncodePoll);

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
        (self.inner.init_encode(dst),
         self.zone.init_encode(dst))
    }
}

/*

impl<T, P: Ptr, Z, M> Clone for Bag<T, P, Z, M>
where T: Clone, P: Clone, Z: Clone, M: Clone,
{
    fn clone(&self) -> Self {
        unsafe {
            let cloned_ptr = self.ptr.clone_unchecked::<T>();
            Self {
                marker: PhantomData,
                ptr: self.ptr.clone_unchecked::<T>(),
                metadata: self.metadata.clone(),
                zone: self.zone.clone(),
            }
        }
    }
}

impl<T, P: Ptr, Z> Default for Bag<T, P, Z>
where T: Default, P: Default, Z: Default,
{
    fn default() -> Self {
        let mut value = ManuallyDrop::new(T::default());
        let metadata = T::metadata(&value);

        unsafe {
            Self::from_raw_parts(
                P::alloc_unchecked(&mut value),
                metadata,
                Z::default()
            )
        }
    }
}

// serialization



impl<T: ?Sized + Pointee, P: Ptr, Z> ValidateBlob for Bag<T, P, Z>
where T::Metadata: ValidateBlob,
      P: ValidateBlob,
      Z: ValidateBlob,
{
    type Error = ValidateBlobBagError<P::Error, <T::Metadata as ValidateBlob>::Error, Z::Error>;

    const BLOB_LEN: usize = P::BLOB_LEN + <T::Metadata as ValidateBlob>::BLOB_LEN + Z::BLOB_LEN;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<P>().map_err(ValidateBlobBagError::Ptr)?
            .field::<T::Metadata>().map_err(ValidateBlobBagError::Metadata)?
            .field::<Z>().map_err(ValidateBlobBagError::Zone)?;

        unsafe { Ok(blob.finish()) }
    }
}

impl<Q: Ptr, T: ?Sized + Pointee, P: Ptr, Z> Load<Q> for Bag<T, P, Z>
where T::Metadata: Decode<Q>,
      P: Decode<Q>,
      Z: Decode<Q>,
{
    fn decode_blob<'a>(mut blob: BlobDecoder<'a, Self, Q>) -> Self {
        let r = unsafe {
            Self {
                marker: PhantomData,
                ptr: blob.decode_unchecked(),
                metadata: blob.decode_unchecked(),
                zone: blob.decode_unchecked(),
            }
        };
        blob.finish();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
*/
*/
*/
