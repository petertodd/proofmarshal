use core::convert::TryInto;
use core::num::NonZeroUsize;
use core::mem;

use super::*;

use crate::blob::StructValidator;


#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ValidateSliceError<E> {
    pub idx: usize,
    pub err: E,
}

impl<T: Persist> Persist for [T]
where T::Persist: Sized
{
    type Persist = [T::Persist];
    type Error = ValidateSliceError<T::Error>;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let len = blob.metadata().get().try_into().unwrap();
        let mut blob = blob.validate_struct();
        for idx in 0 .. len {
            blob.field::<T,_>(|err| ValidateSliceError { idx, err })?;
        }

        unsafe { blob.assume_valid() }
    }
}

#[derive(Debug)]
#[non_exhaustive]
#[doc(hidden)]
pub enum ValidateSliceState<S> {
    #[non_exhaustive]
    #[doc(hidden)]
    Value {
        state: S,
        next: NonZeroUsize,
    },

    #[doc(hidden)]
    #[non_exhaustive]
    Done,
}

unsafe impl<'a, Z, T: Validate<'a, Z>> Validate<'a, Z> for [T]
where T::Persist: Sized
{
    type State = ValidateSliceState<T::State>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        if let Some(first) = this.first() {
            ValidateSliceState::Value {
                state: T::validate_children(first),
                next: NonZeroUsize::new(1).unwrap(),
            }
        } else {
            ValidateSliceState::Done
        }
    }

    fn poll<V: PtrValidator<Z>>(this: &'a [T::Persist], state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        loop {
            *state = match state {
                ValidateSliceState::Value { state, next } => {
                    let next = next.get();
                    T::poll(&this[next - 1], state, validator)?;

                    if let Some(value) = &this.get(next) {
                        ValidateSliceState::Value {
                            state: T::validate_children(value),
                            next: NonZeroUsize::new(next + 1).unwrap(),
                        }
                    } else {
                        ValidateSliceState::Done
                    }
                },
                ValidateSliceState::Done => {
                    break Ok(unsafe { mem::transmute(this) })
                },
            }
        }
    }
}

impl<Z, T: Decode<Z>> Load<Z> for [T] {
}

#[cfg(test)]
mod test {
}
