use super::*;

use std::any::type_name;
use std::fmt;

#[derive(Error, Debug)]
#[error("FIXME")]
pub struct DecodeArrayBytesError<E: fmt::Debug, const N: usize> {
    idx: usize,
    err: E,
}

/*
#[derive(Error, Debug)]
#[error("FIXME")]
pub struct ValidateArrayError<E: fmt::Debug, const N: usize> {
    idx: usize,
    err: E,
}

pub struct ArrayValidator<T, const N: usize> {
    idx: usize,
    poll: [T; N],
}
*/

impl<T: Blob, const N: usize> Blob for [T; N] {
    const SIZE: usize = T::SIZE * N;

    type DecodeBytesError = DecodeArrayBytesError<T::DecodeBytesError, N>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let mut dst = dst.write_struct();

        for item in self.iter() {
            dst = dst.write_field(item);
        }

        dst.done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let _ = src;
        todo!()
    }
}

/*
impl<T: ValidatePoll, const N: usize> ValidatePoll for ArrayValidator<T, N> {
    type Ptr = T::Ptr;
    type Error = ValidateArrayError<T::Error, N>;

    fn validate_poll_impl<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
         where V: Validator<Ptr = Self::Ptr>
    {
        todo!()
    }
}
*/


/*

impl<T: ValidateImpl, const N: usize> ValidateImpl for [T; N] {
    type Ptr = T::Ptr;
    type Error = ValidateArrayError<T::Error, N>;
}

impl<'a, T: 'a + Validate<'a>, const N: usize> Validate<'a> for [T; N] {
    type ValidatePoll = ValidateArrayPoll<'a, T, N>;

    fn init_validate(&self) -> Self::ValidatePoll {
        todo!()
    }
}


impl<'a, T: Validate<'a>, const N: usize> fmt::Debug for ValidateArrayPoll<'a, T, N>
where T: fmt::Debug,
      T::ValidatePoll: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("poll", &self.poll)
            .field("remaining", &self.remaining)
            .finish()
    }
}

impl<'a, T: Validate<'a>, const N: usize> ValidatePoll<T::Ptr> for ValidateArrayPoll<'a, T, N> {
    type Error = ValidateArrayError<T::Error, N>;

    fn validate_poll<V>(&mut self, validator: &mut V) -> Poll<Result<(), Self::Error>>
        where V: Validator<Ptr = T::Ptr>
    {
        todo!()
    }
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn encode() {
    }
}
