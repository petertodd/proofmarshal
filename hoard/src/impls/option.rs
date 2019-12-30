use std::any::type_name;
use std::convert;
use std::mem;

use super::*;

use nonzero::NonZero;

use crate::marshal::blob::*;

impl<T: 'static + NonZero + ValidateBlob> ValidateBlob for Option<T> {
    type Error = T::Error;

    fn validate<'a, V: PaddingValidator>(mut blob: BlobCursor<'a, Self, V>)
        -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
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

impl<Z, T: NonZero + Encoded<Z>> Encoded<Z> for Option<T>
where T::Encoded: NonZero
{
    type Encoded = Option<T::Encoded>;
}

impl<'a, Z, T: NonZero + Encode<'a, Z>> Encode<'a, Z> for Option<T>
where T::Encoded: NonZero
{
    type State = Option<T::State>;

    fn save_children(&'a self) -> Self::State {
        self.as_ref().map(T::save_children)
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        match (self, state) {
            (Some(value), Some(state)) => value.poll(state, dumper),
            (None, None) => Ok(dumper),
            _ => panic!("invalid state"),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        match (self, state) {
            (Some(value), Some(state)) => value.encode_blob(state, dst),
            (None, None) => {
                let zeros = [0u8; mem::size_of::<Self::Encoded>()];
                dst.write_bytes(&zeros)?
                   .finish()
            },
            _ => panic!("invalid state"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytes::Bytes;

    use std::convert::TryFrom;
    use std::num::NonZeroU8;

    #[test]
    fn test_validate() {
        let blob = Blob::<Option<NonZeroU8>>::try_from(&[0][..]).unwrap();
        let valid = ValidateBlob::validate(blob.into_cursor()).unwrap().to_ref();
        assert!(valid.is_none());

        for i in 1 .. 255 {
            let buf = [i];
            let blob = Blob::<Option<NonZeroU8>>::try_from(&buf[..]).unwrap();
            let valid = ValidateBlob::validate(blob.into_cursor()).unwrap().to_ref();
            assert_eq!(*valid, Some(NonZeroU8::new(i).unwrap()));
        }
    }
}
