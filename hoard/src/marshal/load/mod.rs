use std::alloc::Layout;
use std::any::type_name;
use std::fmt;
use std::mem;

use owned::IntoOwned;

use crate::pointee::Pointee;

use super::blob;
use super::decode::*;
use super::PtrValidator;

/// `Persist`, but for unsized types.
///
/// Automatically implemented for all `T: Persist`.
pub unsafe trait PersistPointee : Pointee<Metadata: blob::Validate> + IntoOwned {
    type Persist : 'static + ?Sized + Pointee<Metadata=Self::Metadata, LayoutError=Self::LayoutError>
                                    + blob::Validate<Error=<Self as PersistPointee>::Error>;

    type Error : 'static + std::error::Error + Send + Sync;

    /// Converts the persistent version to owned, assuming all internal pointers are valid.
    unsafe fn assume_valid(this: &Self::Persist) -> Self::Owned;

    /// Converts a persistent reference to `&Self`, assuming all internal pointers are valid.
    unsafe fn assume_valid_ref(this: &Self::Persist) -> &Self;
}

unsafe impl<T: Persist> PersistPointee for T {
    type Persist = T::Persist;
    type Error = T::Error;

    unsafe fn assume_valid(this: &Self::Persist) -> Self {
        T::assume_valid(this)
    }

    unsafe fn assume_valid_ref(this: &T::Persist) -> &Self {
        T::assume_valid_ref(this)
    }
}

/// `ValidateChildren` but for `?Sized` types.
pub unsafe trait ValidatePointeeChildren<'a, Z> : PersistPointee {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error>;
}

unsafe impl<'a, Z, T> ValidatePointeeChildren<'a, Z> for T
where T: Persist + ValidateChildren<'a, Z>,
{
    type State = T::State;

    fn validate_children(this: &'a T::Persist) -> T::State {
        T::validate_children(this)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a T::Persist, state: &mut T::State, validator: &V) -> Result<(), V::Error>
    {
        T::poll(this, state, validator)
    }
}

pub trait Load<Z> : PersistPointee + for<'a> ValidatePointeeChildren<'a, Z>
{}

impl<Z, T: Persist + Decode<Z>> Load<Z> for T {
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
