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

pub trait Validate : Pointee {
    type Error : ValidationError;

    fn validate<B>(blob: B) -> Result<B::Ok, B::Error>
        where B: BlobValidator<Self>;
}

pub unsafe trait Load<Z: Zone> : Validate + Owned {
    type ValidateChildren : ValidateChildren<Z>;
    fn validate_children(&self) -> Self::ValidateChildren;
}

pub trait ValidateChildren<Z: Zone> {
    fn poll<V>(&mut self, ptr_validator: &V) -> Result<(), V::Error>
        where V: PtrValidator<Z>;
}

impl<Z: Zone> ValidateChildren<Z> for () {
    fn poll<V>(&mut self, _: &V) -> Result<(), V::Error>
        where V: PtrValidator<Z>
    {
        Ok(())
    }
}

pub unsafe trait PtrValidator<Z: Zone> {
    type Error;

    fn validate_ptr<'a, T: ?Sized + Load<Z>>(&self, ptr: &'a FatPtr<T, Z::Persist>)
        -> Result<Option<T::ValidateChildren>, Self::Error>;
}
