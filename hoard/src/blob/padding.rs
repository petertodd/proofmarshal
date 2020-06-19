//! Padding byte validation.

use thiserror::Error;

mod sealed {
    pub trait Sealed {}
}

pub trait ValidatePadding : Copy + sealed::Sealed {
    type Error : std::error::Error + 'static + Send + Sync;

    fn validate_padding(&self, buf: &[u8]) -> Result<(), Self::Error>;
}

/// Ignores non-zero padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IgnorePadding;

impl sealed::Sealed for IgnorePadding {}

impl ValidatePadding for IgnorePadding {
    type Error = !;

    fn validate_padding(&self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Checks for non-zero padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CheckPadding;

impl sealed::Sealed for CheckPadding {}

/// Error returned when padding is non-zero.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("Non-zero padding")]
#[non_exhaustive]
pub struct Error;

impl ValidatePadding for CheckPadding {
    type Error = Error;

    fn validate_padding(&self, buf: &[u8]) -> Result<(), Self::Error> {
        if buf.iter().all(|b| *b == 0) {
            Ok(())
        } else {
            Err(Error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_padding() {
        assert_eq!(IgnorePadding.validate_padding(&[1,2,3,4]), Ok(()));
        assert_eq!(IgnorePadding.validate_padding(&[]), Ok(()));
    }

    #[test]
    fn check_padding() {
        assert_eq!(CheckPadding.validate_padding(&[]), Ok(()));
        assert_eq!(CheckPadding.validate_padding(&[0,0,0]), Ok(()));
        assert_eq!(CheckPadding.validate_padding(&[1,2,3,4]), Err(Error));
    }
}
