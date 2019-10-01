use core::any::type_name;
use core::fmt;

use crate::pointee::slice::SliceLen;

use super::*;

unsafe impl<T,A: Arena> Persist<A> for [T]
where T: Persist<A>
{
    type Error = VerifySliceError<T,A>;

    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        let len = unver.valid_metadata();
        let mut unver = unver.verify_struct(arena);

        for idx in 0 .. len.get() {
            unver = unver.field::<T>().map_err(|err| VerifySliceError { len, idx, err })?;
        }

        unver.finish()
    }
}

pub struct VerifySliceError<T: Persist<A>, A: Arena> {
    len: SliceLen<T>,
    idx: usize,
    err: T::Error,
}

impl<T: Persist<A>, A: Arena> fmt::Debug for VerifySliceError<T,A>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("len", &self.len)
            .field("idx", &self.idx)
            .field("err", &self.err)
            .finish()
    }
}

unsafe impl<T,A: Arena> Persist<A> for SliceLen<T> {
    /// Any slice length is allowed in terms of what metadata we allow.
    type Error = !;
    fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        Ok(unsafe { unver.assume_init() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::convert::TryFrom;

    use super::super::primitive::ValidateBoolError;

    #[test]
    fn verify_err() {
        let len = SliceLen::try_from(3).unwrap();

        let buf = [0u8,1,2];
        let unver = Unverified::<[bool]>::new_unsized(&buf, len);

        let e = <[bool] as Persist>::verify(unver, &()).unwrap_err();

        assert_eq!(e.len, len);
        assert_eq!(e.idx, 2);
        assert_eq!(e.err, ValidateBoolError(2));
    }
}
