use super::*;

/// Returned when validating a bool fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoolValidateError(u8);

unsafe impl Persist for bool {
    type Error = BoolValidateError;

    #[inline]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        match maybe[0] {
            0 | 1 => unsafe { Ok(maybe.assume_valid()) },
            x => Err(BoolValidateError(x)),
        }
    }

    #[inline]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        dst.write_bytes([if *self { 1 } else { 0 }]);
        dst.done()
    }
}

/// Returned when validation of a slice (or array) fails.
pub struct ValidateSliceError<T: Persist> {
    idx: usize,
    err: T::Error,
}

impl<T: Persist> fmt::Debug for ValidateSliceError<T>
where T::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("idx", &self.idx)
            .field("err", &self.err)
            .finish()
    }
}

macro_rules! array_impls {
    ($($N:literal)+) => {
        $(
            unsafe impl<T: Persist> Persist for [T;$N] {
                type Error = ValidateSliceError<T>;

                #[inline]
                fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
                    let mut fields = maybe.validate_fields();
                    for idx in 0 .. $N {
                        fields = fields.field::<T>()
                                       .map_err(|err| ValidateSliceError { idx, err })?;
                    }

                    unsafe {
                        Ok(fields.assume_valid())
                    }
                }

                #[inline]
                fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
                    for field in self {
                        dst.write(field);
                    }
                    dst.done()
                }
            }
        )+
    }
}

array_impls! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

macro_rules! impl_all_valid {
    ($( $t:ty, )+) => {
        $(
            unsafe impl Persist for $t {
                type Error = core::convert::Infallible;

                #[inline(always)]
                fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
                    unsafe {
                        Ok(maybe.assume_valid())
                    }
                }

                #[inline]
                fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
                    let buf = unsafe { slice::from_raw_parts(self as *const _ as *const u8,
                                                             mem::size_of::<Self>()) };
                    dst.write_bytes(buf);
                    dst.done()
                }
            }
        )+
    }
}

impl_all_valid! {
    (), u8, i8,
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::num::NonZeroU8;

    #[test]
    fn test() {
        let u = MaybeValid::<NonZeroU8>::default();

        assert_eq!(&u[..], &[0]);
    }

    #[test]
    fn arrays() {
        let orig = &[true; 10];
        let maybe = MaybeValid::from_valid_ref(orig);
        let r = <[bool;10] as Persist>::validate(maybe).unwrap();
        assert_eq!(orig.as_ptr(), r.as_ptr());
    }
}
