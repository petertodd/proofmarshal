/// A `Box` that is generic over the type of `Ptr`.

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
use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::ptr::*;

/// A `Box` that is generic over the type of `Ptr`.
///
/// # Metadata Type Parameter
///
/// Currently in Rust the use of associated types from traits makes a struct invariant over the
/// type. We however want to be covariant over `T`, so the metadata is provided as a third type
/// parameter.
#[repr(C)]
pub struct Bag<T: ?Sized + Pointee, P: Ptr, M: 'static = <T as Pointee>::Metadata> {
    marker: PhantomData<T>,
    ptr: P,
    metadata: M,
}

impl<T: ?Sized + Pointee, P: Ptr, M: 'static> Drop for Bag<T, P, M> {
    fn drop(&mut self) {
        // We have to be generic over M because Rust requires Drop to be implemented for all even
        // though the only way to create a Bag is for M to be T::Metadata
        let metadata: &dyn Any = &self.metadata;
        let metadata: &T::Metadata = metadata.downcast_ref()
                                             .expect("metadata to be correct type");

        // SAFETY: ptr being valid is an invariant we uphold
        unsafe { self.ptr.dealloc::<T>(*metadata) };
    }
}


/// Error returned when validation of a `Blob<Bag>` fails.
#[derive(Debug, Error)]
#[error("bag failed")]
pub enum ValidateBagBlobError<PtrError: Error, MetadataError: Error, LayoutError: Error> {
    Ptr(PtrError),
    Metadata(MetadataError),
    Layout(LayoutError),
}

unsafe impl<T: ?Sized + ValidateBlob, P: Ptr> ValidateBlob for Bag<T, P> {
    type BlobError = ValidateBagBlobError<<P as ValidateBlob>::BlobError, <T::Metadata as ValidateBlob>::BlobError, T::LayoutError>;

    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(P::blob_layout().extend(T::Metadata::blob_layout()))
    }

    fn validate_blob<'a>(blob: Blob<'a, Self>, ignore_padding: bool) -> Result<ValidBlob<'a, Self>, Self::BlobError> {
        let mut fields = blob.validate_fields(ignore_padding);

        fields.validate_blob::<P>().map_err(ValidateBagBlobError::Ptr)?;
        let metadata_blob = fields.validate_blob::<T::Metadata>().map_err(ValidateBagBlobError::Metadata)?;
        let metadata = metadata_blob.as_value().clone();

        T::try_blob_layout(metadata).map_err(ValidateBagBlobError::Layout)?;

        unsafe { Ok(fields.finish()) }
    }
}

impl<T: ?Sized + ValidateBlob, P: Ptr> Load for Bag<T, P> {
    type Ptr = P;

    fn decode_blob(blob: ValidBlob<Self>, zone: &<Self::Ptr as Ptr>::BlobZone) -> Self {
        let mut fields = blob.decode_fields(zone);

        let r = unsafe {
            Self {
                marker: PhantomData,
                ptr: fields.decode_unchecked(),
                metadata: fields.decode_unchecked(),
            }
        };
        fields.finish();
        r
    }
}

impl<Q: Ptr, T: ?Sized + Saved<Q>, P: Ptr> Saved<Q> for Bag<T, P> {
    type Saved = Bag<T::Saved, Q>;
}

/// The poller used to save a `Bag`.
pub struct BagSavePoll<Q: Ptr, T: ?Sized + SavePtr<P, Q>, P: Ptr> {
    metadata: T::Metadata,
    state: State<Q::Persist, T::SavePtrPoll, P::Persist>,
}

enum State<QPersist, TSavePoll, PPersist> {
    Clean(PPersist),
    Dirty(TSavePoll),
    Done(QPersist),
}


impl<Q: Ptr, T: ?Sized + SavePtr<P, Q>, P: Ptr> Save<Q> for Bag<T, P>
{
    type SavePoll = BagSavePoll<Q, T, P>;

    fn init_save(&self) -> Self::SavePoll {
        BagSavePoll {
            metadata: self.metadata,

            // SAFETY: self.ptr being valid is an invariant we uphold
            state: match unsafe { self.ptr.try_get_dirty_unchecked::<T>(self.metadata) } {
                       Ok(value) => State::Dirty(value.init_save_ptr()),
                       Err(persist_ptr) => State::Clean(persist_ptr),
                   },
        }
    }
}

impl<Q: Ptr, T: ?Sized + SavePtr<P, Q>, P: Ptr> EncodeBlob for BagSavePoll<Q, T, P> {
    type Target = Bag<T::Saved, Q>;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        if let State::Done(q_persist) = self.state {
            todo!()
        } else {
            panic!("polling incomplete")
        }
    }
}

impl<Q: Ptr, T: ?Sized + SavePtr<P, Q>, P: Ptr> SavePoll for BagSavePoll<Q, T, P> {
    type SrcPtr = P;
    type DstPtr = Q;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>,
    {
        loop {
            self.state = match &mut self.state {
                State::Clean(persist_ptr) => {
                    match saver.try_save::<T>(persist_ptr, self.metadata)? {
                        Ok(dst_persist) => State::Done(dst_persist),
                        Err(value_poller) => State::Dirty(value_poller),
                    }
                },
                State::Dirty(value_poller) => {
                    value_poller.save_poll(saver)?;

                    let persist_ptr = saver.finish_save(value_poller)?;
                    State::Done(persist_ptr)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }
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

/*
impl<T: ?Sized + Pointee, Z, P: Ptr> BlobSize for Bag<T, Z, P> {
}

impl<V: Copy, T: ?Sized + Pointee, Z, P: Ptr> ValidateBlob<V> for Bag<T, Z, P> {
    type Error = <Own<T, P> as ValidateBlob<V>>::Error;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}
*/

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
