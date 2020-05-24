use std::any::{Any, type_name};
use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ptr;

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
pub struct Bag<T: ?Sized + Pointee, P: Ptr, Z, M: 'static = <T as Pointee>::Metadata> {
    marker: PhantomData<T>,
    ptr: P,
    metadata: M,
    zone: Z,
}

impl<T: ?Sized + Pointee, P: Ptr, Z, M> Drop for Bag<T, P, Z, M> {
    fn drop(&mut self) {
        let metadata: &dyn Any = &self.metadata;
        let metadata = metadata.downcast_ref()
                               .expect("metadata to be the correct type");
        unsafe { self.ptr.dealloc::<T>(*metadata) }
    }
}

impl<T: ?Sized + Pointee, P: Ptr, Z> Bag<T, P, Z> {
    pub fn new_in(value: impl Take<T>, mut alloc: impl Alloc<Ptr=P, Zone=Z>) -> Self {
        let zone = alloc.zone();
        value.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            let ptr = alloc.alloc_unchecked(src);
            Self::from_raw_parts(ptr, metadata, zone)
        })
    }
}

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

impl<T: ?Sized + Pointee, P: Ptr, Z, M> Bag<T, P, Z, M> {
    pub unsafe fn from_raw_parts(ptr: P, metadata: M, zone: Z) -> Self {
        Self { marker: PhantomData, ptr, metadata, zone }
    }

    pub fn into_raw_parts(self) -> (P, M, Z) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.ptr),
             ptr::read(&this.metadata),
             ptr::read(&this.zone))
        }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateBlobBagError<P: fmt::Debug, M: fmt::Debug, Z: fmt::Debug> {
    Ptr(P),
    Metadata(M),
    Zone(Z),
}

impl<T: ?Sized + Pointee, P: Ptr, Z, M> ValidateBlob for Bag<T, P, Z, M>
where P: ValidateBlob,
      M: ValidateBlob,
      Z: ValidateBlob,
{
    type Error = ValidateBlobBagError<P::Error, M::Error, Z::Error>;

    const BLOB_LEN: usize = P::BLOB_LEN + M::BLOB_LEN + Z::BLOB_LEN;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<P>().map_err(ValidateBlobBagError::Ptr)?;
        blob.field::<M>().map_err(ValidateBlobBagError::Metadata)?;
        blob.field::<Z>().map_err(ValidateBlobBagError::Zone)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Y, T: ?Sized + Pointee, P: Ptr, Z> Decode<Y> for Bag<T, P, Z>
where T::Metadata: Decode<Y>,
      P: Decode<Y>,
      Z: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        let r = unsafe {
            Self {
                marker: PhantomData,
                ptr: blob.field_unchecked(),
                metadata: blob.field_unchecked(),
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
pub struct EncodeBagState<'a, R, T: ?Sized, TState, ZState> {
    zone_state: ZState,
    pointee_state: PointeeState<'a, R, T, TState>,
}

#[derive(Debug)]
enum PointeeState<'a, R, T: ?Sized, TState> {
    Ready,
    Poll {
        value: &'a T,
        value_state: TState,
    },
    SaveBlob {
        value: &'a T,
        value_state: TState,
    },
    Done(R),
}

impl<'a, Q: 'a, R: Ptr, T: 'a + ?Sized + Pointee, P: Ptr, Z> Encode<'a, Q, R> for Bag<T, P, Z>
where R: Primitive,
      T: Save<'a, Q, R>,
      Z: Encode<'a, Q, R>,
      P: std::borrow::Borrow<Q>,
{
    type State = EncodeBagState<'a, R, T, T::State, Z::State>;

    fn init_encode_state(&'a self) -> Self::State {
        EncodeBagState {
            zone_state: self.zone.init_encode_state(),
            pointee_state: PointeeState::Ready,
        }
    }

    fn encode_poll<D>(&'a self, state: &mut Self::State, mut dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        dst = self.zone.encode_poll(&mut state.zone_state, dst)?;

        loop {
            state.pointee_state = match &mut state.pointee_state {
                PointeeState::Ready => {
                    match unsafe { dst.try_save_ptr::<T>(self.ptr.borrow(), self.metadata) } {
                        Ok(r_ptr) => PointeeState::Done(r_ptr),
                        Err(value) => PointeeState::Poll {
                            value_state: value.init_save_state(),
                            value,
                        },
                    }
                },
                PointeeState::Poll { value, value_state } => {
                    dst = value.save_poll(value_state, dst)?;

                    PointeeState::SaveBlob {
                        value,
                        value_state: mem::replace(value_state, value.init_save_state()),
                    }
                },
                PointeeState::SaveBlob { value, value_state } => {
                    let (d, r_ptr) = dst.save_ptr::<T>(value, value_state)?;
                    dst = d;
                    PointeeState::Done(r_ptr)
                },
                PointeeState::Done(_) => break Ok(dst),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob
    {
        if let PointeeState::Done(r_ptr) = &state.pointee_state {
            dst.write_primitive(r_ptr)?
               .write_primitive(&self.metadata)?
               .write(&self.zone, &state.zone_state)?
               .done()
        } else {
            panic!()
        }
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
