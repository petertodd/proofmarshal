use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU8, Ordering};
use std::convert::TryFrom;

use thiserror::Error;

use hoard::zone::{Missing, Zone};
use hoard::prelude::*;

use proofmarshal_core::commit::Digest;

use crate::merklesum::MerkleSum;
use crate::tree::{
    SumTree, DynSumTree,
    height::{Height, NonZeroHeight},
};

pub mod length;
use self::length::*;

pub struct SumMMR<T, S: Copy, Z: Zone, L: ?Sized + GetLength = Length> {
    marker: PhantomData<OwnedPtr<T,Z>>,
    flags: AtomicU8,
    tips_digest: UnsafeCell<Digest>,
    tips: MaybeUninit<Z::Ptr>,
    sum: UnsafeCell<S>,
    len: L,
}

pub type MMR<T, Z, L = Length> = SumMMR<T, (), Z, L>;
pub type DynSumMMR<T, S, Z> = SumMMR<T, S, Z, DynLength>;


pub struct Cell<T, S: Copy, Z: Zone, L: ?Sized + GetLength = NonZeroLength> {
    tree: SumTree<T, S, Z, ()>,
    rest: SumMMR<T, S, Z, ()>,
    len: L,
}

impl<T, S: Copy, Z: Zone> SumMMR<T, S, Z> {
    pub fn new() -> Self where S: Default {
        Self {
            marker: PhantomData,
            flags: 0.into(),
            tips_digest: Digest::default().into(),
            tips: MaybeUninit::uninit(),
            sum: S::default().into(),
            len: Length::default(),
        }
    }
}

enum TipRef<'a, T, S: Copy, Z: Zone> {
    None,
    Tree(&'a DynSumTree<T, S, Z>),
    Cell(&'a Cell<T, S, Z, DynLength>),
}

impl<T, S: Copy, Z: Zone, L: ?Sized + GetLength> SumMMR<T, S, Z> {
}

impl<T, S: Copy, Z: Zone, L: ?Sized + GetLength> Cell<T, S, Z, L> {
    pub fn tree(&self) -> &DynSumTree<T, S, Z> {
        let height = NonZeroHeight::try_from(self.len().trailing_zeros() as usize).unwrap();
        todo!()
    }

    pub fn rest(&self) -> &DynSumMMR<T, S, Z> {
        let len = self.len();
        let smallest_pow = 1 << len.trailing_zeros();
        let rest_len = len - smallest_pow;

        todo!()
    }

    pub fn len(&self) -> u64 {
        let len = self.len.get().get();
        assert!(len.count_ones() > 1);
        len
    }
}

bitflags::bitflags! {
    pub struct Flags: u8 {
        const DIGEST_DIRTY  = 0b0001;
        const DIGEST_LOCKED = 0b0010;
        const SUM_DIRTY     = 0b0100;
        const SUM_LOCKED    = 0b1000;
    }
}

impl<T, S: Copy, Z: Zone, L: ?Sized + GetLength> Drop for SumMMR<T, S, Z, L> {
    fn drop(&mut self) {
        match self.len.get().get() {
            0 => {
                // nothing to do
            },
            1 => {
                todo!()
            },
            len => {
                todo!()
            }
        }
    }
}

impl From<Flags> for AtomicU8 {
    #[inline(always)]
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

impl<T, S: Copy, Z: Zone> Default for SumMMR<T, S, Z>
where S: Default
{
    fn default() -> Self {
        Self::new()
    }
}


impl<T, S: Copy, Z: Zone, L: ?Sized + GetLength> Drop for Cell<T, S, Z, L> {
    fn drop(&mut self) {
        todo!()
    }
}


/*
/// Returned when a push operation fails.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PushError<SumError: std::error::Error> {
    #[error("length overflowed")]
    LengthOverflow,

    #[error("sum overflowed")]
    SumOverflow(SumError),
}

pub struct Tip<T: Fact<Z>, S: MerkleSum<T>, Z: Zone = Missing> {
    tree: SumTree<T, S, Z>,
    next: SumMMR<T, S, Z>,
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::prelude::*;
    use hoard::pile::PileMut;

    #[test]
    fn test() {
        let mut mmr: MMR<u8, PileMut> = MMR::new();
    }
}
