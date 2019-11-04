//! Tuple marshalling.

use crate::pile::{self, Layout, Loader, Dumper, Marshal};

use core::cmp;
use core::mem;
use core::fmt;

pub enum LoadTupleError<I: Marshal<Z>, Z: pile::Zone, N> {
    Err(I::Error),
    Next(N),
}

impl<I: Marshal<Z>, Z: pile::Zone, N> fmt::Debug for LoadTupleError<I,Z,N>
where N: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadTupleError::Err(e) => f.debug_tuple("Err")
                                       .field(e)
                                       .finish(),
            LoadTupleError::Next(n) => f.debug_tuple("Next")
                                       .field(n)
                                       .finish(),
        }
    }
}

impl<I: Marshal<Z>, Z: pile::Zone, N> cmp::PartialEq for LoadTupleError<I,Z,N>
where I::Error: PartialEq,
      N: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LoadTupleError::Err(e1), LoadTupleError::Err(e2)) => e1 == e2,
            (LoadTupleError::Next(n1), LoadTupleError::Next(n2)) => n1 == n2,
            _ => false,
        }
    }
}


impl<Z: pile::Zone, A: Marshal<Z>, B: Marshal<Z>>
Marshal<Z> for (A,B)
{
    type Error = LoadTupleError<A, Z, LoadTupleError<B, Z, !>>;

    const LAYOUT: Layout = A::LAYOUT.extend(B::LAYOUT);

    #[inline(always)]
    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        let (a, loader) = loader.read().unwrap();
        let (b, loader) = loader.read().unwrap();
        Ok(((a,b), loader.done()))
    }

    #[inline(always)]
    fn store<D: Dumper<Zone=Z>>(self, dumper: D) -> Result<D::Ok, D::Error> {
        dumper.write(self.0)?
              .write(self.1)?
              .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(<(bool, bool)>::load(&[0,1][..]),
                   Ok(((false, true), ())));

        assert_eq!((false, true).store(vec![]).unwrap(),
                   &[0,1]);
    }
}
