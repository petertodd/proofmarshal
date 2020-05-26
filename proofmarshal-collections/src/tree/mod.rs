use std::borrow::{Borrow, BorrowMut};
use std::cell::{Cell, UnsafeCell};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit, ManuallyDrop};
use std::num::NonZeroU8;
use std::slice;
use std::sync::atomic::{AtomicU8, Ordering};
use std::ptr;
use std::ops;

use thiserror::Error;

use owned::{IntoOwned, Take};

use hoard::prelude::*;
use proofmarshal_core::commit::{Digest, Commit, Verbatim, WriteVerbatim};

use crate::merklesum::MerkleSum;

pub mod height;
use self::height::*;

mod flags;
use self::flags::*;

mod std_impls;
mod marshal_impls;
mod pointee_impls;
mod drop_impls;

#[cfg(test)]
mod tests;

#[repr(C)]
struct SumTreeData<T, S, P> {
    marker: PhantomData<T>,
    flags: AtomicU8,
    tip_digest: UnsafeCell<Digest>,
    sum: UnsafeCell<S>,
    tip: P,
}

/// Perfect merkle sum tree.
#[repr(C)]
pub struct SumTree<T, S, P: Ptr, Z = ()> {
    data: SumTreeData<T, S, P>,
    zone: Z,
    height: Height,
}

#[repr(C)]
pub struct SumTreeDyn<T, S, P: Ptr, Z = ()> {
    data: SumTreeData<T, S, P>,
    zone: Z,
    height: HeightDyn,
}

/// Perfect merkle tree.
pub type Tree<T, P, Z = ()> = SumTree<T, (), P, Z>;

pub type TreeDyn<T, P, Z = ()> = SumTreeDyn<T, (), P, Z>;

#[repr(C)]
pub struct Inner<T, S, P: Ptr> {
    left:   ManuallyDrop<SumTreeData<T, S, P>>,
    right:  ManuallyDrop<SumTreeData<T, S, P>>,
    height: NonZeroHeight,
}

#[repr(C)]
pub struct InnerDyn<T, S, P: Ptr> {
    left:   ManuallyDrop<SumTreeData<T, S, P>>,
    right:  ManuallyDrop<SumTreeData<T, S, P>>,
    height: NonZeroHeightDyn,
}

#[derive(Debug)]
pub enum TipRef<'a, T, S, P: Ptr> {
    Leaf(&'a T),
    Inner(&'a InnerDyn<T, S, P>),
}

#[derive(Debug)]
pub enum Tip<Leaf, Inner> {
    Leaf(Leaf),
    Inner(Inner),
}

#[derive(Debug, Error)]
pub enum JoinError<SumError: std::error::Error> {
    #[error("height mismatch")]
    HeightMismatch,

    #[error("height overflow")]
    HeightOverflow,

    #[error("sum overflow")]
    SumOverflow(SumError),
}

impl<T, S: MerkleSum<T>, P: Ptr, Z> SumTree<T, S, P, Z> {
    pub fn new_leaf_in(value: T, mut alloc: impl Alloc<Ptr=P, Zone=Z>) -> Self {
        Self {
            data: SumTreeData {
                flags: (Flags::DIGEST_DIRTY | Flags::SUM_DIRTY).into(),
                marker: PhantomData,
                tip_digest: Default::default(),
                sum: S::MAX.into(),
                tip: alloc.alloc_ptr(value),
            },
            zone: alloc.zone(),
            height: Height::new(0).unwrap(),
        }
    }

    pub fn try_join_in(self, rhs: Self, mut alloc: impl Alloc<Zone=Z, Ptr=P>) -> Result<Self, JoinError<S::Error>> {
        let tip = Inner::new(self, rhs)?;
        let height: Height = tip.height.into();
        let sum = tip.sum();

        Ok(Self {
            data: SumTreeData {
                flags: Flags::DIGEST_DIRTY.into(),
                marker: PhantomData,
                tip_digest: Default::default(),
                sum: sum.into(),
                tip: alloc.alloc_ptr::<InnerDyn<T, S, P>,_>(tip),
            },
            zone: alloc.zone(),
            height,
        })
    }
}

impl<T, S, P: Ptr, Z> SumTree<T, S, P, Z> {
    fn into_raw_parts(self) -> (SumTreeData<T, S, P>, Z, Height) {
        let this = ManuallyDrop::new(self);
        unsafe {
            (ptr::read(&this.data),
             ptr::read(&this.zone),
             ptr::read(&this.height))
        }
    }

    fn into_data(self) -> SumTreeData<T, S, P> {
        let (data, _, _) = self.into_raw_parts();
        data
    }
}

impl<T, S, P: Ptr, Z> SumTreeDyn<T, S, P, Z>
where T: Load<P>,
      S: Decode<P>,
      P: Decode<P>,
{
    #[inline]
    pub fn get<'a>(&'a self, idx: usize) -> Option<Ref<'a, T>>
        where Z: Get<P>
    {
        self.try_get(idx).into_ok()
    }

    #[inline]
    pub fn try_get<'a>(&'a self, idx: usize) -> Result<Option<Ref<'a, T>>, Z::Error>
        where Z: TryGet<P>
    {
        self.try_get_in(idx, &self.zone)
    }

    #[inline]
    pub fn try_get_in<'a, Y: TryGet<P>>(&'a self, idx: usize, zone: &Y) -> Result<Option<Ref<'a, T>>, Y::Error> {
        if idx < self.len() {
            match self.try_get_tip_in(zone)? {
                Tip::Leaf(leaf) => Ok(Some(leaf)),
                Tip::Inner(Ref::Ref(inner)) => inner.try_get_in(idx, zone),
                Tip::Inner(Ref::Owned(inner)) => {
                    inner.try_get_in(idx, zone).map(|r| match r {
                        None => None,
                        Some(Ref::Owned(owned)) => Some(Ref::Owned(owned)),
                        Some(Ref::Ref(_)) => unreachable!(),
                    })
                }
            }
        } else {
            Ok(None)
        }
    }

    fn try_get_tip_in<'a, Y: TryGet<P>>(&'a self, zone: &Y) -> Result<Tip<Ref<'a, T>, Ref<'a, InnerDyn<T, S, P>>>, Y::Error> {
        unsafe {
            if let Ok(height) = NonZeroHeight::try_from(self.height()) {
                zone.try_get_unchecked::<InnerDyn<T, S, P>>(&self.data.tip, height)
                    .map(Tip::Inner)
            } else {
                zone.try_get_unchecked::<T>(&self.data.tip, T::make_sized_metadata())
                    .map(Tip::Leaf)
            }
        }
    }
}

impl<T, S, P: Ptr> InnerDyn<T, S, P>
where T: Load<P>,
      S: Decode<P>,
      P: Decode<P>,
{
    #[inline]
    pub fn try_get_in<'a, Z: TryGet<P>>(&'a self, idx: usize, zone: &Z) -> Result<Option<Ref<'a, T>>, Z::Error> {
        if idx < self.len() / 2 {
            self.left().try_get_in(idx, zone)
        } else if idx < self.len() {
            let idx = idx - (self.len() / 2);
            self.right().try_get_in(idx, zone)
        } else {
            Ok(None)
        }
    }
}


impl<T, S, P: Ptr, Z> SumTreeDyn<T, S, P, Z> {
    #[inline]
    pub fn len(&self) -> usize {
        1 << u8::from(self.height())
    }

    /// Gets the height of the tree.
    pub fn height(&self) -> Height {
        self.height.to_owned()
    }

    #[inline]
    pub fn sum(&self) -> S
        where S: MerkleSum<T>
    {
        if let Some(sum) = self.try_sum() {
            *sum
        } else {
            self.fix_dirty_sum()
        }
    }

    /// Tries to get the sum, if already calculated.
    pub fn try_sum(&self) -> Option<&S> {
        self.data.try_sum()
    }

    fn fix_dirty_sum(&self) -> S
        where S: MerkleSum<T>
    {
        let sum = match self.get_dirty_tip().expect("dirty tip pointer") {
            TipRef::Leaf(leaf) => S::from_item(leaf),
            TipRef::Inner(inner) => inner.sum(),
        };

        match self.data.try_lock(Flags::SUM_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.data.sum.get() = sum;
                }

                self.data.unlock(Flags::SUM_LOCKED, Flags::SUM_DIRTY);

                sum
            },
            Err(old_flags) => {
                todo!("race: {:?}", old_flags)
            },
        }
    }

    #[inline]
    pub fn tip_digest(&self) -> Digest
        where S: MerkleSum<T>
    {
        if let Some(digest) = self.try_tip_digest() {
            digest
        } else {
            self.fix_dirty_tip_digest()
        }
    }

    /// Tries to get the sum, if already calculated.
    pub fn try_tip_digest(&self) -> Option<Digest> {
        self.data.try_tip_digest()
    }

    fn fix_dirty_tip_digest(&self) -> Digest
    {
        Digest::default()
        /*
        let sum = match self.get_dirty_tip().expect("dirty tip pointer") {
            TipRef::Leaf(leaf) => S::from_item(leaf),
            TipRef::Inner(inner) => inner.sum(),
        };

        match self.data.try_lock(Flags::SUM_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.data.sum.get() = sum;
                }

                self.data.unlock(Flags::SUM_LOCKED, Flags::SUM_DIRTY);

                sum
            },
            Err(old_flags) => {
                todo!("race: {:?}", old_flags)
            },
        }
        */
    }

    fn get_dirty_tip<'a>(&'a self) -> Result<TipRef<'a, T, S, P>, P::Persist> {
        unsafe {
            if let Ok(height) = NonZeroHeight::try_from(self.height()) {
                self.data.tip.try_get_dirty_unchecked(height)
                    .map(TipRef::Inner)
            } else {
                self.data.tip.try_get_dirty_unchecked::<T>(())
                    .map(TipRef::Leaf)
            }
        }
    }
}

impl<T, S, P> SumTreeData<T, S, P> {
    /// Tries to get the sum, if already calculated.
    pub fn try_sum(&self) -> Option<&S> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::SUM_DIRTY) {
            None
        } else {
            unsafe { Some(&*self.sum.get()) }
        }
    }

    /// Tries to get the sum, if already calculated.
    pub fn try_tip_digest(&self) -> Option<Digest> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::DIGEST_DIRTY) {
            None
        } else {
            unsafe { Some(*self.tip_digest.get()) }
        }
    }

    fn load_flags(&self, ordering: Ordering) -> Flags {
        let flags = self.flags.load(ordering);
        match Flags::from_bits(flags) {
            Some(flags) => flags,
            None => {
                unreachable!("invalid flags: {:b}", flags)
            }
        }
    }

    fn try_lock(&self, lock_flag: Flags) -> Result<Flags, Flags> {
        let old_flags = self.flags.fetch_or(lock_flag.bits(), Ordering::SeqCst);
        match Flags::from_bits(old_flags) {
            Some(old_flags) if old_flags.contains(lock_flag) => Err(old_flags),
            Some(old_flags) => Ok(old_flags),
            None => {
                unreachable!("invalid flags: {:b}", old_flags)
            }
        }
    }

    fn unlock(&self, lock_flag: Flags, dirty_bit: Flags) {
        let old_flags = self.flags.fetch_and(!(lock_flag | dirty_bit).bits(), Ordering::SeqCst);
        let old_flags = Flags::from_bits(old_flags).expect("valid flags");
        assert!(old_flags.contains(lock_flag | dirty_bit),
                "{:?}", old_flags);
    }

    /// Sets all dirty bits.
    fn set_dirty(&mut self) {
        *self.flags.get_mut() |= (Flags::DIGEST_DIRTY | Flags::SUM_DIRTY).bits();
    }
}

impl<T, S: MerkleSum<T>, P: Ptr> Inner<T, S, P> {
    pub fn new<Z>(left: SumTree<T, S, P, Z>, right: SumTree<T, S, P, Z>) -> Result<Self, JoinError<S::Error>> {
        if left.height != right.height {
            Err(JoinError::HeightMismatch)
        } else {
            S::try_sum(&left.sum(), &right.sum()).map_err(JoinError::SumOverflow)?;

            match left.height.try_increment() {
                None => Err(JoinError::HeightOverflow),
                Some(height) => {
                    Ok(Inner {
                        left: ManuallyDrop::new(left.into_data()),
                        right: ManuallyDrop::new(right.into_data()),
                        height,
                    })
                }
            }
        }
    }
}

impl<T, S, P: Ptr> InnerDyn<T, S, P> {
    #[inline]
    pub fn len(&self) -> usize {
        1 << u8::from(self.height())
    }

    /// Gets the height of the tree.
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_owned()
    }

    pub fn sum(&self) -> S
        where S: MerkleSum<T>
    {
        S::try_sum(&self.left().sum(), &self.right().sum()).expect("sum to be valid")
    }

    pub fn left(&self) -> &SumTreeDyn<T, S, P, ()> {
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts(&self.left, next_height))
        }
    }

    pub fn left_mut(&mut self) -> &mut SumTreeDyn<T, S, P, ()> {
        self.left.set_dirty();
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(&mut self.left, next_height))
        }
    }

    pub fn right(&self) -> &SumTreeDyn<T, S, P, ()> {
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts(&self.right, next_height))
        }
    }

    pub fn right_mut(&mut self) -> &mut SumTreeDyn<T, S, P, ()> {
        self.right.set_dirty();
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(&mut self.right, next_height))
        }
    }
}
