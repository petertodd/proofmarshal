use core::mem;

use crate::blob;
use crate::pointee::Pointee;

pub unsafe trait Persist : Pointee<Metadata = ()>
where Self::Metadata: blob::Validate
{
    type Persist : 'static + Pointee<Metadata=()> + blob::Validate<Error=<Self as Persist>::Error>;
    type Error : 'static;
}

/// `Persist`, but for unsized types.
///
/// Automatically implemented for all `T: Persist`.
pub unsafe trait PersistPointee : Pointee<Metadata: blob::Validate> {
    type PersistPointee : 'static + Pointee<Metadata=Self::Metadata> + blob::Validate<Error=<Self as PersistPointee>::Error>;
    type Error : 'static;

    type MetadataError : 'static;

    fn validate_metadata(metadata: Self::Metadata) -> Result<usize, Self::MetadataError>;
}

unsafe impl<T: Persist> PersistPointee for T
where T::Metadata: blob::Validate
{
    type PersistPointee = T::Persist;
    type Error = T::Error;
    type MetadataError = !;

    fn validate_metadata(_: ()) -> Result<usize, !> {
        Ok(mem::size_of::<T>())
    }
}

pub unsafe trait ValidateChildren<'a, Z> : Persist {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error>;

    unsafe fn assume_valid(this: &'a Self::Persist) -> Self;
    unsafe fn assume_valid_ref(this: &'a Self::Persist) -> &'a Self;
}

/// `ValidateChildren` but for `?Sized` types.
pub unsafe trait ValidatePointeeChildren<'a, Z> : PersistPointee {
    type State;

    fn validate_children(this: &'a Self::Persist) -> Self::State;

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error>;

    unsafe fn assume_valid_ref(this: &'a Self::Persist) -> &'a Self;
}

impl<'a, Z, T: ValidateChildren<'a, Z>> ValidatePointeeChildren<'a, Z> {
    type State = T::State;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        T::validate_children(this)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        T::poll(this, state, validator)
    }

    unsafe fn assume_valid_ref(this: &'a Self::Persist) -> &'a Self {
        T::assume_valid_ref(this)
    }
}


pub trait PtrValidator<Z> {
    type Error;
}


pub trait Load<Z> : PersistPointee + for<'a> ValidateChildren<'a, Z>
{}

pub trait Decode<Z> : Persist + for<'a> ValidateChildren<'a, Z> {}

impl<Z, T: Persist + Decode<Z>> Load<Z> for T {
}

#[macro_export]
macro_rules! impl_primitive_load {
    ($t:ty) => {
        unsafe impl Persist for $t {
            type Persist = Self;
            type Error = <Self as crate::blob::Validate>::Error;
        }

        unsafe impl<'a, Z> crate::load::ValidateChildren<'a, Z> for $t {
            type State = ();

            fn validate_children(_: &'a Self) -> Self::State {}

            fn poll<V>(_this: &'a Self, _: &mut (), _: &V) -> Result<(), V::Error>
                where V: crate::load::PtrValidator<Z>
            {
                Ok(())
            }

            unsafe fn assume_valid(this: &'a Self::Persist) -> &'a Self {
                ::core::mem::transmute(this)
            }
        }

        impl<Z> crate::load::Decode<Z> for $t {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
