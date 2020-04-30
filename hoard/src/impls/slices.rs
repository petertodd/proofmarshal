use core::fmt;
use core::mem::{self, MaybeUninit};

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

use crate::marshal::load::*;
use crate::marshal::save::*;

#[derive(Error, Debug, PartialEq, Eq)]
#[error("slice validation failed")]
pub struct ValidateSliceError<E: fmt::Debug> {
    idx: usize,
    err: E,
}

impl<E: fmt::Debug + Into<!>> From<ValidateSliceError<E>> for ! {
    fn from(err: ValidateSliceError<E>) -> ! {
        err.err.into()
    }
}

impl<T: ValidateBlob> ValidateBlob for [T] {
    type Error = ValidateSliceError<T::Error>;

    fn validate<'a, V: PaddingValidator>(blob: BlobCursor<'a, Self, V>)
        -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
    {
        /*
        for i in 0 .. N {
            blob.field::<T,_>(|err| ValidateArrayError { idx: i, err })?;
        }

        unsafe { blob.assume_valid() }
        */ todo!()
    }
}

unsafe impl<T: Persist> PersistPointee for [T] {
    type Persist = [T::Persist];
    type Error = <Self::Persist as ValidateBlob>::Error;

    unsafe fn assume_valid(this: &Self::Persist) -> Self::Owned {
        todo!()
    }

    unsafe fn assume_valid_ref(this: &Self::Persist) -> &Self {
        todo!()
    }
}

unsafe impl<'a, Z, T> ValidatePointeeChildren<'a, Z> for [T]
where T: Persist + ValidateChildren<'a, Z>,
{
    type State = !;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        todo!()
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        todo!()
    }
}

impl<Z, T> Load<Z> for [T]
where T: Decode<Z>,
{}

impl<Y, T: Encoded<Y>> Saved<Y> for [T] {
    type Saved = [T::Encoded];
}

impl<'a, Y, T: Encode<'a, Y>> Save<'a, Y> for [T] {
    type State = !;

    fn make_save_state(&'a self) -> Self::State {
        todo!()
    }

    fn save_poll<D: Dumper<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Error> {
        todo!()
    }
}
