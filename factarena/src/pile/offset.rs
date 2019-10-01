use core::marker::PhantomData;
use core::num::NonZeroU64;

use crate::prelude::*;

use crate::util::nonzero::NonZero;
use crate::arena::persist::*;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Offset<'pile> {
    marker: PhantomData<fn() -> &'pile ()>,
    offset: Le<NonZeroU64>,
}

impl Offset<'_> {
    #[inline]
    fn get(self) -> usize {
        self.offset.get().get() as usize
    }
}

impl From<Offset<'_>> for usize {
    #[inline]
    fn from(offset: Offset<'_>) -> Self {
        offset.get()
    }
}

impl From<NonZeroU64> for Offset<'_> {
    #[inline]
    fn from(offset: NonZeroU64) -> Self {
        Self {
            marker: PhantomData,
            offset: offset.into(),
        }
    }
}

unsafe impl NonZero for Offset<'_> {}
unsafe impl<A: Arena> Persist<A> for Offset<'_> {
    type Error = <Le<NonZeroU64> as Persist>::Error;

    #[inline]
    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        unver.verify_struct(arena)
             .field::<Le<NonZeroU64>>()?
             .finish()
    }
}
