use super::*;

use core::any::type_name;
use core::fmt;

use crate::util::nonzero::NonZero;

unsafe impl<T: Persist<A>, A: Arena> Persist<A> for Option<T>
where T: NonZero
{
    type Error = VerifyOptionError<T,A>;

    #[inline]
    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        if unver.iter().all(|x| *x == 0) {
            // all zeros, so None is a valid interpretation
            let r = unsafe { unver.assume_init() };
            assert!(r.is_none());
            Ok(r)
        } else {
            unver.verify_struct(arena)
                 .field::<T>().map_err(|err| VerifyOptionError(err))?
                 .finish()
        }
    }

    /*
    #[inline]
    fn write_canonical_bytes<W: io::Write>(&self, mut w: W) -> io::Result<W> {
        match self {
            None => {
                for i in 0 .. mem::size_of::<Self>() {
                    w.write_all(&[0])?;
                }
                Ok(w)
            },
            Some(v) => v.write_canonical_bytes(w),
        }
    }
    */
}

pub struct VerifyOptionError<T: Persist<A>, A: Arena>(T::Error);

impl<T: Persist<A>, A: Arena> fmt::Debug for VerifyOptionError<T,A>
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
    use core::num::NonZeroU8;

    #[test]
    fn test() {
        let n = NonZeroU8::new(42).unwrap();

        let _opt = Some(n);

        /*
        assert_eq!(Persist::<()>::canonical_bytes(&opt), &[42]);
        assert_eq!(Persist::<()>::canonical_bytes(&None::<NonZeroU8>), &[0]);
        */
    }
}
