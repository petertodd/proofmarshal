//! Padding validation

pub unsafe trait PaddingValidator : Copy {
    type Error;
    fn validate_padding(&self, buf: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckPadding;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IgnorePadding;

unsafe impl PaddingValidator for IgnorePadding {
    type Error = !;

    #[inline(always)]
    fn validate_padding(&self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct PaddingError;

unsafe impl PaddingValidator for CheckPadding {
    type Error = PaddingError;

    #[inline(always)]
    fn validate_padding(&self, buf: &[u8]) -> Result<(), Self::Error> {
        match buf.iter().all(|b| *b == 0) {
            true => Ok(()),
            false => Err(PaddingError),
        }
    }
}

unsafe impl<T: PaddingValidator> PaddingValidator for &'_ T {
    type Error = T::Error;

    #[inline(always)]
    fn validate_padding(&self, buf: &[u8]) -> Result<(), Self::Error> {
        (&**self).validate_padding(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let check = CheckPadding;

        assert_eq!(check.validate_padding(&[]), Ok(()));
        assert_eq!(check.validate_padding(&[0,0,0]), Ok(()));

        assert_eq!(check.validate_padding(&[1]), Err(PaddingError));

        let check = IgnorePadding;
        assert_eq!(check.validate_padding(&[]), Ok(()));
        assert_eq!(check.validate_padding(&[0,0,0]), Ok(()));
        assert_eq!(check.validate_padding(&[1,2,3]), Ok(()));
    }
}
