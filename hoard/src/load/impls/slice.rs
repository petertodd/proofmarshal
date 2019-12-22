use super::*;

use core::any::type_name;
use core::fmt;

pub struct ValidateSliceError<E> {
    idx: usize,
    err: E,
}

impl<T: Validate> Validate for [T] {
    type Error = ValidateSliceError<T::Error>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        todo!()
    }
}

#[cfg(test)]
mod test {
}
