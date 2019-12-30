use std::any::type_name;
use std::convert;

use super::*;

use nonzero::NonZero;

use crate::marshal::blob::*;

impl<T: 'static + NonZero + Validate> Validate for Option<T> {
    type Error = T::Error;

    fn validate<'a, V: Validator>(mut blob: Cursor<'a, Self, V>)
        -> Result<ValidBlob<'a, Self>, blob::Error<Self::Error, V::Error>>
    {
        if blob.iter().all(|b| *b == 0) {
            // None variant
            unsafe { blob.assume_valid() }
        } else {
            blob.field::<T,_>(convert::identity)?;
            unsafe { blob.assume_valid() }
        }
    }
}

unsafe impl<T: NonZero + Persist> Persist for Option<T>
where T::Persist: NonZero
{
    type Persist = Option<T::Persist>;
    type Error = T::Error;
}

unsafe impl<'a, Z, T> ValidateChildren<'a, Z> for Option<T>
where T: NonZero + ValidateChildren<'a, Z>,
      T::Persist: NonZero
{
    type State = Option<T::State>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        this.as_ref().map(T::validate_children)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        match (this, state) {
            (Some(value), Some(value_state)) => T::poll(value, value_state, validator),
            (None, None) => Ok(()),
            _ => panic!("Option::<{}>::poll() called with invalid state", type_name::<T>()),
        }
    }
}

impl<Z, T> Decode<Z> for Option<T>
where T: NonZero + Decode<Z>,
      T::Persist: NonZero
{}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytes::Bytes;

    use std::convert::TryFrom;
    use std::num::NonZeroU8;

    #[test]
    fn test_validate() {
        let blob = Blob::<Option<NonZeroU8>>::try_from(&[0][..]).unwrap();
        let valid = Validate::validate(blob.into_cursor()).unwrap().to_ref();
        assert!(valid.is_none());

        for i in 1 .. 255 {
            let buf = [i];
            let blob = Blob::<Option<NonZeroU8>>::try_from(&buf[..]).unwrap();
            let valid = Validate::validate(blob.into_cursor()).unwrap().to_ref();
            assert_eq!(*valid, Some(NonZeroU8::new(i).unwrap()));
        }
    }
}
