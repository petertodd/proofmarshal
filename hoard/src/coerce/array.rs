use super::*;

unsafe impl<U, T: TryCoerce<U>, const N: usize> TryCoerce<[U; N]> for [T; N] {
    type Error = TryCoerceArrayError<T::Error, N>;

    fn try_coerce_ptr(this: &Self) -> Result<*const [U; N], Self::Error> {
        for (idx, item) in this.iter().enumerate() {
            T::try_coerce_ptr(item)
              .map_err(|err| TryCoerceArrayError { idx, err })?;
        }

        assert_eq!(Layout::new::<Self>(), Layout::new::<[U; N]>(),
                   "{} can-not implement TryCoerce<{}>: layouts differ",
                   type_name::<Self>(), type_name::<T>());
        Ok(this as *const _ as *const _)
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TryCoerceArrayError<E, const N: usize> {
    pub idx: usize,
    pub err: E,
}

impl<E, const N: usize> From<TryCoerceArrayError<E, N>> for !
where E: Into<!>
{
    fn from(err: TryCoerceArrayError<E,N>) -> ! {
        Into::<!>::into(err.err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coerce() {
        let b = [true, false, true];
        let n: [u8;3] = b.coerce();
        assert_eq!(n, [1, 0, 1]);
    }

    #[test]
    fn test_try_coerce() {
        let n = [0u8, 1u8, 0u8];
        let b: [bool;3] = n.try_coerce().unwrap();
        assert_eq!(b, [false, true, false]);

        let n = [0u8, 1u8, 2u8];
        assert_eq!(TryCoerce::<[bool; 3]>::try_coerce(n).unwrap_err(),
                   TryCoerceArrayError { idx: 2, err: TryCoerceBoolError });
    }
}
