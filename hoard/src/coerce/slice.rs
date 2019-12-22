use super::*;

use core::slice;
use core::mem;

/// Error when trying to cast a slice fails.
///
/// Implements `Into<!>` when `E: Into<!>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct TryCastSliceError<E> {
    pub idx: usize,
    pub err: E,
}

impl<E: Into<!>> From<TryCastSliceError<E>> for ! {
    fn from(err: TryCastSliceError<E>) -> ! {
        err.err.into()
    }
}

unsafe impl<T: TryCastRef<U>, U> TryCastRef<[U]> for [T] {
    type Error = TryCastSliceError<T::Error>;

    fn try_cast_ref(&self) -> Result<&[U], Self::Error> {
        for (idx, item) in self.iter().enumerate() {
            item.try_cast_ref()
                .map_err(|err| TryCastSliceError { idx, err })?;
        }

        Ok(unsafe {
            slice::from_raw_parts(self.as_ptr() as *const U,
                                  self.len())
        })
    }
}
unsafe impl<T: TryCastMut<U>, U> TryCastMut<[U]> for [T] {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cast_ref_identity() {
        let s: &[u8] = &[1,2,3];
        let s2: &[u8] = s.as_cast_ref();

        let s: &mut [u8] = &mut [1,2,3];
        let s2: &mut [u8] = s.as_cast_mut();
    }
}
