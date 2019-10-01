use super::*;

use core::any::type_name;
use core::fmt;
use core::mem;

impl<T,U,A: ?Sized> Validate<[U],A> for [T]
where T: Validate<U,A>
{
    type Error = ValidateSliceError<T,U,A>;

    fn validate<'a>(unver: MaybeValid<'a, Self, [U]>, arena: &A) -> Result<Valid<'a, Self, [U]>, Self::Error> {
        for (idx, u) in unver.iter().enumerate() {
            let unver = MaybeValid::new(u, T::make_sized_metadata());
            if let Err(err) = T::validate(unver, arena) {
                return Err(ValidateSliceError { idx, err });
            }
        }
        Ok(unsafe { unver.assume_init() })
    }
}

pub struct ValidateSliceError<T: Validate<U,A>, U: ?Sized, A: ?Sized> {
    idx: usize,
    err: T::Error,
}

impl<T: Validate<U,A>, A: ?Sized, U: ?Sized> fmt::Debug for ValidateSliceError<T,U,A>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("idx", &self.idx)
            .field("err", &self.err)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
