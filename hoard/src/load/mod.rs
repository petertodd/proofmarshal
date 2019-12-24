//! In-place data validation and loading.

use core::any::Any;
use core::fmt;

use owned::Owned;

use crate::{
    blob::Blob,
    pointee::Pointee,
    zone::{Zone, FatPtr},
};

use crate::blob::BlobValidator;

pub mod impls;

mod error;
pub use self::error::*;

pub trait ValidateBlob : Pointee {
    type Error : 'static;

    fn validate_blob<B>(blob: B) -> Result<B::Ok, B::Error>
        where B: BlobValidator<Self>;
}

pub trait Persist : Pointee {
    type Persist : 'static + ValidateBlob;
}

/// # Safety
///
/// The metadata must be compatible.
pub unsafe trait PersistPtr : Pointee {
    type Persist : 'static + ?Sized + ValidateBlob;

    fn coerce_metadata_into_persist(metadata: Self::Metadata) -> <Self::Persist as Pointee>::Metadata;
    fn coerce_metadata_from_persist(metadata: <Self::Persist as Pointee>::Metadata) -> Self::Metadata;
}

unsafe impl<T: Persist> PersistPtr for T {
    type Persist = T::Persist;

    fn coerce_metadata_into_persist(_: Self::Metadata) -> <Self::Persist as Pointee>::Metadata {
        <Self::Persist as Pointee>::make_sized_metadata()
    }

    fn coerce_metadata_from_persist(metadata: <Self::Persist as Pointee>::Metadata) -> Self::Metadata {
        Self::make_sized_metadata()
    }
}

pub unsafe trait ValidateChildren<'a, Z = !> : Persist {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error>;
}

pub unsafe trait ValidatePtrChildren<'a, Z = !> : PersistPtr {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error>;
}

unsafe impl<'a, Z, T: ValidateChildren<'a, Z>> ValidatePtrChildren<'a, Z> for T {
    type State = T::State;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        T::validate_children(this)
    }
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        T::poll(this, state, validator)
    }
}

pub trait Decode<Z> : Persist + for<'a> ValidateChildren<'a, Z> {
}

pub trait Load<Z> : Pointee + Owned + PersistPtr + for<'a> ValidatePtrChildren<'a, Z> {
    type Error : 'static;
}

impl<Z, T: Decode<Z>> Load<Z> for T {
    type Error = <T::Persist as ValidateBlob>::Error;
}

/*
pub unsafe trait ValidatePtr<'a, Z = !> {
    type Persist : 'static + ?Sized + ValidateBlob<Error=Self::Error>;
    type Error : 'static;
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error>;
}

unsafe impl<'a, Z, T: Validate<'a, Z>> ValidatePtr<'a, Z> for T {
    type Persist = T::Persist;
    type State = T::State;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        T::validate_children(this)
    }
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        T::poll(this, state, validator)
    }
}
*/


pub unsafe trait PtrValidator<Z> {
    type Error;

    fn validate_ptr<'a, T: ?Sized + Pointee>(&self, ptr: &'a FatPtr<T::Persist, Z::Persist>)
        -> Result<Option<&'a T::Persist>, Self::Error>
    where Z: Zone,
          T: ValidatePtrChildren<'a, Z>;
}
