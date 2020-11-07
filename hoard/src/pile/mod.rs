use crate::blob::Blob;
use crate::load::{Load, LoadIn, LoadRefIn, MaybeValid};
use crate::ptr::{TryGet, AsZone};
use crate::owned::Take;

pub use crate::offset::{Offset, OffsetMut, Error, SliceGetBlobError};

#[derive(Debug, Default)]
pub struct Pile<B> {
    inner: B,
}

impl<B> Pile<B> {
    pub fn new(inner: B) -> Self {
        Self { inner }
    }
}

impl<B: AsRef<[u8]>> Pile<B> {
    pub fn try_get_tip<'p, T>(&'p self) -> Result<MaybeValid<T>, Error<SliceGetBlobError>>
        where T: Load,
              &'p [u8]: AsZone<T::Zone>,

    {
        let mapping: &'p [u8] = self.inner.as_ref();

        let offset = mapping.len().saturating_sub(T::Blob::SIZE);
        let offset = Offset::new(offset as u64, mapping);

        unsafe {
            offset.try_take::<T>(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bag::Bag;

    #[test]
    fn pile_try_get_tip_trivial() -> Result<(), Box<dyn std::error::Error>> {
        let pile = Pile::new(vec![42]);

        let tip = pile.try_get_tip::<u8>()?.trust();
        assert_eq!(tip, 42);

        Ok(())
    }

    #[test]
    fn pile_try_get_tip() -> Result<(), Box<dyn std::error::Error>> {
        let pile = Pile::new(vec![
            42,
            0,0,0,0,0,0,0,0,
        ]);

        let tip: Bag<u8, OffsetMut<&[u8]>> = pile.try_get_tip()?.trust();
        assert_eq!(tip.get(), &42);

        Ok(())
    }
}
