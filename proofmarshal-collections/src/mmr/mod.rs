use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU8, Ordering};

use thiserror::Error;

use hoard::zone::{Missing, Zone};
use hoard::prelude::*;
use proofmarshal_core::fact::Fact;
use proofmarshal_core::commit::Digest;

use crate::tree::{SumTree, Flags};
use crate::merklesum::MerkleSum;

pub mod length;
use self::length::*;

pub struct SumMMR<T: Fact<Z>, S: MerkleSum<T>, Z: Zone = Missing, L: ?Sized + GetLength = Length> {
    marker: PhantomData<Tip<T, S, Z>>,
    flags: AtomicU8,
    tips_digest: UnsafeCell<Digest>,
    tips: MaybeUninit<Z::Ptr>,
    sum: S,
    len: L,
}

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone> Default for SumMMR<T, S, Z>
where S: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone> SumMMR<T, S, Z> {
    /// Creates a new, empty, `SumMMR`.
    pub fn new() -> Self where S: Default {
        Self {
            marker: PhantomData,
            flags: 0.into(),
            tips_digest: Digest::default().into(),
            tips: MaybeUninit::uninit(),
            sum: S::default(),
            len: Length::new(0).unwrap(),
        }
    }

    pub fn len(&self) -> Length {
        todo!()
    }

    pub fn push_in(&mut self, value: T, zone: &Z) -> Result<(), PushError<S::Error>>
        where Z: Alloc
    {
        todo!()
    }
}

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
