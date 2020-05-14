use super::*;

use thiserror::Error;

impl Load for ! {
    type Error = !;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<R> Saved<R> for ! {
    type Saved = !;
}

impl<Q, R> Save<'_, Q, R> for ! {
    type State = !;

    fn init_save_state(&self) -> ! {
        *self
    }

    fn save_poll<D: SavePtr<Q, R>>(&self, _state: &mut Self::State, _dumper: D) -> Result<D, D::Error> {
        match *self {}
    }

    fn save_blob<W: SaveBlob>(&self, _state: &Self::State, _dst: W) -> Result<W::Done, W::Error> {
        match *self {}
    }

    fn encode_blob<W: WriteBlob>(&self, _state: &Self::State, _dst: W) -> Result<W::Done, W::Error> {
        match *self {}
    }
}

impl Primitive for ! {
}


/*
#[derive(Debug, Error, PartialEq, Eq)]
#[error("the ! type has no valid representations")]
pub struct ValidateNeverError;

impl ValidateBlob for ! {
    type Error = ValidateNeverError;

    fn validate<'a, V>(_blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        Err(ValidateNeverError.into())
    }
}

unsafe impl crate::marshal::decode::Persist for ! {
    type Persist = Self;
    type Error = ValidateNeverError;
}

unsafe impl<'a, Z> ValidateChildren<'a, Z> for ! {
    type State = !;

    fn validate_children(this: &Self) -> Self::State {
        match *this {}
    }

    fn poll<V>(this: &Self, _: &mut !, _: &V) -> Result<(), V::Error>
        where V: PtrValidator<Z>
    {
        match *this {}
    }
}

impl<Z> Decode<Z> for ! {}

impl<Z> Encoded<Z> for ! {
    type Encoded = Self;
}

impl<'a, Z> Encode<'a, Z> for ! {
    type State = !;

    fn make_encode_state(&self) -> Self::State {
        match *self {}
    }

    fn encode_poll<D: Dumper<Z>>(&self, _state: &mut Self::State, _dumper: D) -> Result<D, D::Error> {
        match *self {}
    }

    fn encode_blob<W: WriteBlob>(&self, _state: &Self::State, _dst: W) -> Result<W::Ok, W::Error> {
        match *self {}
    }
}
impl Primitive for ! {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;

    #[test]
    fn validate_never_blob() {
        let blob = Blob::<!>::try_from(&[][..]).unwrap();

        assert_eq!(ValidateBlob::validate(blob.into_cursor()).unwrap_err(),
                   BlobError::Error(ValidateNeverError));
    }
}
*/
