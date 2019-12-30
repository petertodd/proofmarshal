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

pub trait Persist : Pointee {
    type Persist : 'static + ?Sized + Pointee<Metadata=Self::Metadata>;
    type Error : 'static + fmt::Debug;

    fn validate_blob<B>(blob: B) -> Result<B::Ok, B::Error>
        where B: BlobValidator<Self>;

    fn blob_size(metadata: Self::Metadata) -> usize {
        assert_eq!(Self::layout(metadata), Self::Persist::layout(metadata));
        Self::layout(metadata).size()
    }

    unsafe fn assume_valid(this: &Self::Persist) -> &Self {
        &*Self::make_fat_ptr(this as *const _ as *const _, Self::Persist::metadata(this))
    }
}

pub unsafe trait Validate<'a, Z = !> : Persist {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;
    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error>;
}

pub trait Decode<Z> : Owned + Persist<Persist: Sized> + for<'a> Validate<'a, Z>
{}

pub trait Load<Z> : Owned + Persist + for<'a> Validate<'a, Z> {
}

impl<Z, T: Decode<Z>> Load<Z> for T
where Self::Persist: Sized
{}

pub unsafe trait PtrValidator<Z> {
    type Error;

    fn validate_ptr<'a, T: 'a + ?Sized + Pointee>(&self, ptr: &'a FatPtr<T::Persist, Z::Persist>)
        -> Result<Option<&'a T::Persist>, Self::Error>
    where Z: 'a + Zone,
          T: Validate<'a, Z>;
}
