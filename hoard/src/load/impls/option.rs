use core::any::type_name;
use core::fmt;
use core::mem;

use nonzero::NonZero;

use crate::blob::Blob;

use super::*;

impl<T: NonZero + Validate> Validate for Option<T> {
    type Error = OptionError<T::Error>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        unsafe { blob.validate_option::<T,_>(OptionError) }
    }
}

#[derive(Debug)]
pub struct OptionError<E>(pub E);

impl<E: ValidationError> ValidationError for OptionError<E> {
}

unsafe impl<T: NonZero + Load<Z>, Z: Zone> Load<Z> for Option<T> {
    type ValidateChildren = Option<T::ValidateChildren>;

    fn validate_children(&self) -> Self::ValidateChildren {
        self.as_ref().map(T::validate_children)
    }
}

impl<T: ValidateChildren<Z>, Z: Zone> ValidateChildren<Z> for Option<T> {
    fn poll<V: PtrValidator<Z>>(&mut self, ptr_validator: &V) -> Result<(), V::Error> {
        match self {
            None => Ok(()),
            Some(inner) => inner.poll(ptr_validator),
        }
    }
}

