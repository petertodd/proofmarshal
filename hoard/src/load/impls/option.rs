use core::any::type_name;
use core::fmt;
use core::mem;

use nonzero::NonZero;

use crate::blob::Blob;

use super::*;

impl<T: NonZero + Persist> Persist for Option<T> {
    type Persist = Option<T::Persist>;
}

impl<T: ValidateBlob> ValidateBlob for Option<T> {
    type Error = OptionError<T::Error>;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        unsafe { blob.validate_option::<T,_>(OptionError) }
    }
}

#[derive(Debug)]
pub struct OptionError<E>(pub E);

unsafe impl<'a, Z, T: NonZero + ValidateChildren<'a, Z>> ValidateChildren<'a, Z> for Option<T> {
    type State = Option<T::State>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        this.as_ref().map(T::validate_children)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V)
        -> Result<&'a Option<T>, V::Error>
    {
        match (this, state) {
            (Some(value), Some(state)) => {
                T::poll(value, state, validator)?;
                Ok(unsafe { mem::transmute(this) })
            },
            (None, None) => Ok(unsafe { mem::transmute(this) }),
            _ => unreachable!(),
        }
    }
}

impl<Z, T: NonZero + Decode<Z>> Decode<Z> for Option<T> {
}
