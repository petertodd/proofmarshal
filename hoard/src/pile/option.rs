//! Tuple marshalling.

use crate::pile::{self, Layout, Loader, Dumper, Marshal};

use core::cmp;
use core::mem;
use core::fmt;

#[derive(PartialEq, Eq)]
pub enum LoadOptionError<T: Marshal<Z>, Z: pile::Zone> {
    Discriminant(u8),
    Value(T::Error),
    Padding(Z::PaddingError),
}

impl<Z: pile::Zone, T: Marshal<Z>> fmt::Debug for LoadOptionError<T,Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadOptionError::Discriminant(x) => f.debug_tuple("Discriminant").field(x).finish(),
            LoadOptionError::Value(e) => f.debug_tuple("Value").field(e).finish(),
            LoadOptionError::Padding(e) => f.debug_tuple("Padding").field(e).finish(),
        }
    }
}

impl<Z: pile::Zone, T: Marshal<Z>> Marshal<Z> for Option<T>
{
    type Error = LoadOptionError<T,Z>;

    const LAYOUT: Layout = Layout::new(1 + T::LAYOUT.len());

    #[inline(always)]
    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        let (discriminant, loader) = loader.read_bytes([0]);
        match discriminant[0] {
            0 => {
                Ok((None,
                    loader.verify_padding(T::LAYOUT.len())
                          .map_err(|e| LoadOptionError::Padding(e))?
                          .done(),
                ))
            },
            1 => {
                let (v, loader) = loader.read().map_err(|e| LoadOptionError::Value(e))?;
                Ok((Some(v), loader.done()))
            },
            x => Err(LoadOptionError::Discriminant(x)),
        }
    }

    #[inline(always)]
    fn store<D: Dumper<Zone=Z>>(self, dumper: D) -> Result<D::Ok, D::Error> {
        match self {
            None => {
                dumper.write_bytes(&[0])?
                      .write_padding(T::LAYOUT.len())?
                      .finish()
            },
            Some(v) => {
                dumper.write_bytes(&[1])?
                      .write(v)?
                      .finish()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(<Option<bool>>::load(&[0,0][..]),
                   Ok((None, ())));
        assert_eq!(<Option<bool>>::load(&[1,0][..]),
                   Ok((Some(false), ())));
        assert_eq!(<Option<bool>>::load(&[1,1][..]),
                   Ok((Some(true), ())));
    }
}
