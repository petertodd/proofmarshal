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
use crate::ptr::*;
use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::primitive::*;

#[derive(Debug)]
#[repr(C)]
pub struct Bag<T: ?Sized + Pointee, P: Ptr, Z = (), M: 'static = <T as Pointee>::Metadata> {
    inner: Own<T, P, M>,
    zone: Z,
}

impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z> {
    pub fn new_in(value: impl Take<T>, mut alloc: impl Alloc<Ptr=P, Zone=Z>) -> Self {
        Self {
            inner: alloc.alloc_own(value),
            zone: alloc.zone(),
        }
    }
}

/*
impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z>
where T: Load<Z>,
{
    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where Z: Get<P>
    {
        unsafe { self.zone.get_unchecked::<T>(&self.ptr, self.metadata) }
    }

    pub fn take(self) -> T::Owned
        where Z: Get<P>
    {
        let (ptr, metadata, zone) = self.into_raw_parts();

        unsafe { zone.take_unchecked::<T>(ptr, metadata) }
    }

    pub fn get_mut<'a>(&'a mut self) -> &'a mut T
        where Z: GetMut<P>
    {
        unsafe { self.zone.get_mut_unchecked::<T>(&mut self.ptr, self.metadata) }
    }
}
*/

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

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateBagBlobError<OwnError: Error, ZoneError: Error> {
    Own(OwnError),
    Zone(ZoneError),
}

impl<T: ?Sized + Pointee, P: Ptr, Z, M: 'static> ValidateBlob for Bag<T, P, Z, M>
where P: ValidateBlob,
      M: ValidateBlob,
      Z: ValidateBlob,
{
    type Error = ValidateBagBlobError<<Own<T, P, M> as ValidateBlob>::Error, Z::Error>;

    const BLOB_LEN: usize = <Own<T, P, M> as ValidateBlob>::BLOB_LEN + Z::BLOB_LEN;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<Own<T, P, M>>().map_err(ValidateBagBlobError::Own)?;
        blob.field::<Z>().map_err(ValidateBagBlobError::Zone)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Y, T: ?Sized + Pointee, P: Ptr, Z> Decode<Y> for Bag<T, P, Z>
where P: Decode<Y>,
      Z: Decode<Y>,
      T::Metadata: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        let r = unsafe {
            Self {
                inner: blob.field_unchecked(),
                zone: blob.field_unchecked(),
            }
        };
        blob.finish();
        r
    }
}

unsafe impl<T: ?Sized + Pointee, P: Ptr, Z, M> Persist for Bag<T, P, Z, M>
where P: Persist,
      Z: Persist,
      M: Persist,
{}

impl<R: Ptr, T: ?Sized + Pointee, P: Ptr, Z, M> Encoded<R> for Bag<T, P, Z, M>
where R: Primitive,
      T: Saved<R>,
      Z: Encoded<R>,
      M: Primitive,
{
    type Encoded = Bag<T::Saved, R, Z::Encoded, M>;
}

#[derive(Debug)]
pub struct EncodeBagState<OwnState, ZoneState> {
    inner_state: OwnState,
    zone_state: ZoneState,
}

/*
impl<'a, Q: 'a, R: 'a + Ptr, T: 'a + ?Sized + Pointee, P: Ptr, Z> Encode<'a, Q, R> for Bag<T, P, Z>
where R: Primitive,
      T: Save<'a, Q, R>,
      Z: Encode<'a, Q, R>,
      P: std::borrow::Borrow<Q>,
{
    type State = EncodeBagState<<Own<T, P> as Encode<'a, Q, R>>::State, Z::State>;
    //type State = EncodeBagState<crate::ptr::own::EncodeOwnState<'a, R, T, T::State>, Z::State>;
    //type State = EncodeBagState<(), Z::State>;

    fn init_encode_state(&self) -> Self::State {
        /*
        EncodeBagState {
            inner_state: self.inner.init_encode_state(),
            zone_state: self.zone.init_encode_state(),
        }
        */ todo!()
    }

    fn encode_poll<D>(&self, state: &mut Self::State, mut dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob
    {
        todo!()
    }
}
*/

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
