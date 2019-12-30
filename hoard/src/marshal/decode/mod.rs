use std::alloc::Layout;
use std::any::type_name;
use std::fmt;
use std::mem;

use crate::marshal::blob;
use crate::pointee::Pointee;

use super::PtrValidator;

pub unsafe trait Persist : Sized {
    type Persist : 'static + blob::ValidateBlob<Error=<Self as Persist>::Error>;
    type Error : 'static + std::error::Error + Send + Sync;

    unsafe fn assume_valid(this: &Self::Persist) -> Self {
        assert_correct_persist_impl::<Self>();
        mem::transmute_copy(this)
    }

    unsafe fn assume_valid_ref(this: &Self::Persist) -> &Self {
        assert_correct_persist_impl::<Self>();
        &*(this as *const _ as *const _)
    }
}

/// Asserts that a `Persist` implementation is correct.
#[inline(always)]
pub fn assert_correct_persist_impl<T: Persist>() {
    assert_eq!(Layout::new::<T::Persist>(), Layout::new::<T>(),
               "incorrect implementation of Persist for {}", type_name::<T>());
    assert_eq!(mem::align_of::<T>(), 1,
               "incorrect implementation of Persist for {}", type_name::<T>());
}

pub unsafe trait ValidateChildren<'a, Z> : Persist {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error>;
}

pub trait Decode<Z> : Sized + Persist + for<'a> ValidateChildren<'a, Z> {}

#[macro_export]
macro_rules! impl_decode_for_primitive {
    ($t:ty) => {
        unsafe impl crate::marshal::decode::Persist for $t {
            type Persist = Self;
            type Error = <Self as crate::marshal::blob::ValidateBlob>::Error;
        }

        unsafe impl<'a, Z> crate::marshal::decode::ValidateChildren<'a, Z> for $t {
            type State = ();

            fn validate_children(_: &'a Self) -> Self::State {}

            fn poll<V>(_this: &'a Self, _: &mut (), _: &V) -> Result<(), V::Error>
                where V: crate::marshal::PtrValidator<Z>
            {
                Ok(())
            }
        }

        impl<Z> crate::marshal::decode::Decode<Z> for $t {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
