use super::*;

#[derive(Error, Debug)]
#[error("FIXME")]
pub enum DecodeOptionBytesError<E: std::fmt::Debug> {
    Discriminant,
    Padding,
    Value(E),
}

impl<T: Blob> Blob for Option<T> {
    const SIZE: usize = 1 + T::SIZE;

    type DecodeBytesError = DecodeOptionBytesError<T::DecodeBytesError>;

    fn decode_bytes(_src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        todo!()
    }

    fn encode_bytes<'a>(&self, _dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        todo!()
    }
}

/*
pub struct OptionValidator<T>(Option<T>);

impl<T: ValidatePoll> ValidatePoll for OptionValidator<T> {
    type Ptr = T::Ptr;
    type Error = T::Error;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = Self::Ptr>
    {
        match &mut self.0 {
            None => Ok(()).into(),
            Some(inner) => inner.validate_poll_impl(validator),
        }
    }
}
*/
