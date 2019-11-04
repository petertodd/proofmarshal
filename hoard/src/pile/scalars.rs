//! Pile marshalling for scalar types.

use crate::pile::{self, Layout, Loader, Dumper, Marshal};

use core::mem;

impl<Z: pile::Zone> Marshal<Z> for () {
    type Error = !;

    const LAYOUT: Layout = Layout::new(0);

    #[inline(always)]
    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        Ok(((), loader.done()))
    }

    #[inline(always)]
    fn store<D>(self, dumper: D) -> Result<D::Ok, D::Error>
        where D: Dumper<Zone=Z>
    {
        dumper.finish()
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct LoadBoolError(u8);

impl<Z: pile::Zone> Marshal<Z> for bool {
    type Error = LoadBoolError;

    const LAYOUT: Layout = Layout::new(1);

    #[inline(always)]
    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        let (b, loader) = loader.read_bytes([0]);
        match b[0] {
            0 => Ok((false, loader.done())),
            1 => Ok((true, loader.done())),
            x => Err(LoadBoolError(x))
        }
    }

    #[inline(always)]
    fn store<D>(self, dumper: D) -> Result<D::Ok, D::Error>
        where D: Dumper<Zone=Z>
    {
        dumper.write_bytes(&[self as u8])?
              .finish()
    }
}

macro_rules! impl_ints {
    ($( $t:ty, )+) => {
        $(
            impl<Z: pile::Zone> Marshal<Z> for $t {
                type Error = !;

                const LAYOUT: Layout = Layout::new(mem::size_of::<$t>());

                #[inline(always)]
                fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
                    where L: Loader<Zone=Z>
                {
                    let (b, loader) = loader.read_bytes(Default::default());
                    Ok((Self::from_le_bytes(b), loader.done()))
                }

                #[inline(always)]
                fn store<D: Dumper<Zone=Z>>(self, dumper: D) -> Result<D::Ok, D::Error> {
                    dumper.write_bytes(&self.to_le_bytes()[..])?
                          .finish()
                }
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marshal_bool() {
        assert_eq!(bool::load(&[0][..]),
                   Ok((false, ())));
        assert_eq!(bool::load(&[1][..]),
                   Ok((true, ())));
        assert_eq!(bool::load(&[2][..]),
                   Err(LoadBoolError(2)));

        assert_eq!(false.store(vec![]).unwrap(),
                   &[0]);
        assert_eq!(true.store(vec![]).unwrap(),
                   &[1]);
    }
}
