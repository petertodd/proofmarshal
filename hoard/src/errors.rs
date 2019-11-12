use core::marker::PhantomData;

pub use super::scalars::*;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateNonZeroNumError<T>(PhantomData<T>);

impl<T> ValidateNonZeroNumError<T> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MaybeValidFromSliceError(pub(crate) ());
