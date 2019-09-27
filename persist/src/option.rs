//! `Option<T>` with `T: NonZero + Persist`
//!
//! We can persist these types because of Rust's non-zero optimization.

use super::*;

use core::alloc::Layout;
use core::any::type_name;
use core::fmt;

use nonzero::NonZero;

unsafe impl<T: Persist> Persist for Option<T>
where T: NonZero
{
    type Error = OptionValidateError<T>;

    #[inline]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        assert_eq!(Layout::new::<T>(), Layout::new::<Option<T>>());

        if maybe.iter().all(|x| *x == 0) {
            // all zeros, so None is a valid interpretation
            let r = unsafe { maybe.assume_valid() };
            assert!(r.is_none());
            Ok(r)
        } else {
            let maybe = maybe.validate_fields()
                             .field::<T>().map_err(|err| OptionValidateError(err))?;

            unsafe { Ok(maybe.assume_valid()) }
        }
    }

    #[inline]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        match self {
            None => dst.write_zeros(mem::size_of::<T>()),
            Some(value) => dst.write(value),
        }
        dst.done()
    }
}

pub struct OptionValidateError<T: Persist>(pub T::Error);

impl<T: Persist> fmt::Debug for OptionValidateError<T>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(type_name::<Self>())
            .field(&self.0)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Le, Persist};

    use core::num::NonZeroU32;

    #[test]
    fn test() {
        let n = Le::new(NonZeroU32::new(0x1234_5678_u32).unwrap());
        let mut opt = Some(n);

        assert_eq!(opt.canonical_bytes(),
                   [0x78, 0x56, 0x34, 0x12]);

        opt.take();
        assert_eq!(opt.canonical_bytes(),
                   [0,0,0,0]);
    }
}
