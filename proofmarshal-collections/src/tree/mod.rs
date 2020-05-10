use std::alloc::Layout;
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

use thiserror::Error;

use owned::{IntoOwned, Take};

use hoard::prelude::*;
use proofmarshal_core::commit::{Digest, Commit, Verbatim, WriteVerbatim};

use crate::merklesum::MerkleSum;

pub mod height;
use self::height::*;

/// Perfect merkle sum tree.
#[repr(C)]
pub struct SumTree<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight = Height> {
    marker: PhantomData<T>,
    flags: AtomicU8,
    tip_digest: UnsafeCell<Digest>,
    sum: UnsafeCell<S>,
    tip: P,
    height: H,
}

pub type DynSumTree<T, S, P> = SumTree<T, S, P, DynHeight>;

pub type Tree<T, P, H> = SumTree<T, (), P, H>;
pub type DynTree<T, P> = SumTree<T, (), P, DynHeight>;


#[repr(C)]
pub struct Inner<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight = NonZeroHeight> {
    left:  ManuallyDrop<SumTree<T,S,P,()>>,
    right: ManuallyDrop<SumTree<T,S,P,()>>,
    height: H,
}
pub type DynInner<T, S, P> = Inner<T, S, P, DynNonZeroHeight>;


#[derive(Debug)]
enum Tip<'a, T, S: MerkleSum<T>, P: Ptr> {
    Inner(&'a DynInner<T, S, P>),
    Leaf(&'a T),
}

#[derive(Debug)]
enum TipMut<'a, T, S: MerkleSum<T>, P: Ptr> {
    Inner(&'a mut DynInner<T, S, P>),
    Leaf(&'a mut T),
}

bitflags::bitflags! {
    pub struct Flags: u8 {
        const DIGEST_DIRTY  = 0b0001;
        const DIGEST_LOCKED = 0b0010;
        const SUM_DIRTY     = 0b0100;
        const SUM_LOCKED    = 0b1000;
    }
}

impl From<Flags> for AtomicU8 {
    #[inline(always)]
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

#[derive(Debug, Error)]
pub enum JoinError<SumError: std::fmt::Debug> {
    #[error("height mismatch")]
    HeightMismatch,

    #[error("height overflow")]
    HeightOverflow,

    #[error("sum overflow")]
    SumOverflow(SumError),
}

impl<T, S: MerkleSum<T>, P: Ptr> SumTree<T,S,P> {
    pub fn new_leaf(value: T) -> Self
        where P: Default
    {
        let bag = P::alloc(value);

        Self {
            flags: (Flags::DIGEST_DIRTY | Flags::SUM_DIRTY).into(),
            marker: PhantomData,
            tip_digest: Digest::default().into(),
            sum: S::MAX.into(),
            tip: bag.into_inner().raw,
            height: Height::new(0).unwrap(),
        }
    }

    pub fn try_join(self, rhs: Self) -> Result<Self, JoinError<S::Error>>
        where P: Default
    {
        let tip = Inner::new(self, rhs)?;
        let height: Height = tip.height.into();
        let sum = tip.sum();

        let tip: Bag<DynInner<T, S, P>, P> = P::alloc(tip);

        Ok(Self {
            flags: (Flags::DIGEST_DIRTY).into(),
            marker: PhantomData,
            tip_digest: Digest::default().into(),
            sum: sum.into(),
            tip: tip.into_inner().raw,
            height,
        })
    }

    /// Strips the height.
    fn strip_height(self) -> SumTree<T, S, P, ()> {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: H should be Copy anyway, but easier to just drop it.
        unsafe { ptr::drop_in_place(&mut this.height) };

        // SAFETY: SumTree is #[repr(C)]
        unsafe { mem::transmute_copy::<
            ManuallyDrop<SumTree<T,S,P>>,
                         SumTree<T,S,P,()>,
            >(&this)
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> SumTree<T, S, P, H> {
    /// Gets the height of the tree.
    pub fn height(&self) -> Height {
        self.height.get()
    }

    pub fn len(&self) -> usize {
        1 << u8::from(self.height())
    }

    #[inline]
    pub fn sum(&self) -> S {
        if let Some(sum) = self.try_sum() {
            sum
        } else {
            self.fix_dirty_sum()
        }
    }

    /// Tries to get the sum, if already calculated.
    pub fn try_sum(&self) -> Option<S> {
        if mem::size_of::<S>() == 0 {
            // Fast path for S = ()
            unsafe { Some(MaybeUninit::zeroed().assume_init()) }
        } else {
            let flags = self.load_flags(Ordering::Relaxed);
            if flags.contains(Flags::SUM_DIRTY) {
                None
            } else {
                unsafe { Some(*self.sum.get()) }
            }
        }
    }

    /// Tries to get the tip digest, if already calculated.
    pub fn try_tip_digest(&self) -> Option<Digest> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::DIGEST_DIRTY) {
            None
        } else {
            unsafe { Some(*self.tip_digest.get()) }
        }
    }

    fn fix_dirty_sum(&self) -> S {
        let sum = match self.get_dirty_tip().expect("dirty tip pointer") {
            Tip::Leaf(leaf) => S::from_item(leaf),
            Tip::Inner(inner) => inner.sum(),
        };

        match self.try_lock(Flags::SUM_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.sum.get() = sum;
                }

                self.unlock(Flags::SUM_LOCKED, Flags::SUM_DIRTY);

                sum
            },
            Err(old_flags) => {
                todo!("race")
            },
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
        let old_flags = self.flags.fetch_or(lock_flag.bits, Ordering::SeqCst);
        match Flags::from_bits(old_flags) {
            Some(old_flags) if old_flags.contains(lock_flag) => Err(old_flags),
            Some(old_flags) => Ok(old_flags),
            None => {
                unreachable!("invalid flags: {:b}", old_flags)
            }
        }
    }

    fn unlock(&self, lock_flag: Flags, dirty_bit: Flags) {
        let old_flags = self.flags.fetch_and(!(lock_flag | dirty_bit).bits, Ordering::SeqCst);
        let old_flags = Flags::from_bits(old_flags).expect("valid flags");
        assert!(old_flags.contains(lock_flag | dirty_bit),
                "{:?}", old_flags);
    }

    fn get_dirty_tip<'a>(&'a self) -> Option<Tip<'a, T, S, P>>
    {
        match NonZeroHeight::try_from(self.height.get()) {
            Ok(inner_height) => {
                unsafe {
                    self.tip.try_get_dirty_unchecked(inner_height)
                            .map(Tip::Inner)
                }
            },
            Err(_) => {
                unsafe {
                    self.tip.try_get_dirty_unchecked(())
                            .map(Tip::Leaf)
                }
            }
        }
    }

    /// Sets all dirty bits.
    fn set_dirty(&mut self) {
        *self.flags.get_mut() |= (Flags::DIGEST_DIRTY | Flags::SUM_DIRTY).bits;
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> SumTree<T, S, P, H> {
    #[inline]
    pub fn tip_digest(&self) -> Digest {
        if let Some(digest) = self.try_tip_digest() {
            digest
        } else {
            self.fix_dirty_tip_digest()
        }
    }

    fn fix_dirty_tip_digest(&self) -> Digest
    {
        /*
        let tip_digest = match self.get_tip_ptr() {
            TipPtr::Leaf(leaf_ptr) => {
                let leaf = Z::try_get_dirty(&leaf_ptr).expect("dirty tip pointer");
                leaf.commit().cast()
            },
            TipPtr::Inner(inner_ptr) => {
                let inner = Z::try_get_dirty(&inner_ptr).expect("dirty tip pointer");

                inner.commit().cast()
            },
        };

        match self.try_lock(Flags::DIGEST_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.tip_digest.get() = tip_digest;
                }

                self.unlock(Flags::DIGEST_LOCKED, Flags::DIGEST_DIRTY);

                tip_digest
            },
            Err(old_flags) => {
                todo!("race")
            },
        }
        */ todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr> Inner<T, S, P, NonZeroHeight> {
    pub fn new(left: SumTree<T, S, P>, right: SumTree<T, S, P>) -> Result<Self, JoinError<S::Error>> {
        if left.height != right.height {
            Err(JoinError::HeightMismatch)
        } else {
            S::try_sum(&left.sum(), &right.sum()).map_err(JoinError::SumOverflow)?;

            match left.height.try_increment() {
                None => Err(JoinError::HeightOverflow),
                Some(height) => {
                    Ok(Inner {
                        left: ManuallyDrop::new(left.strip_height()),
                        right: ManuallyDrop::new(right.strip_height()),
                        height,
                    })
                }
            }
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> Inner<T, S, P, H> {
    pub fn sum(&self) -> S {
        S::try_sum(&self.left.sum(), &self.right.sum()).expect("sum to be valid")
    }

    pub fn height(&self) -> NonZeroHeight {
        NonZeroHeight::try_from(self.height.get()).expect("inner node to have non-zero height")
    }

    pub fn left(&self) -> &DynSumTree<T,S,P> {
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts(&self.left, next_height))
        }
    }

    pub fn left_mut(&mut self) -> &mut DynSumTree<T,S,P> {
        self.left.set_dirty();
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(&mut self.left, next_height))
        }
    }

    pub fn right(&self) -> &DynSumTree<T, S, P> {
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts(&self.right, next_height))
        }
    }

    pub fn right_mut(&mut self) -> &mut DynSumTree<T, S, P> {
        self.right.set_dirty();
        let next_height = self.height().decrement().into();
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(&mut self.right, next_height))
        }
    }
}


// ----- hoard impls --------

// ----- Pointee ------

unsafe impl<T, S: MerkleSum<T>, P: Ptr> Pointee for DynSumTree<T, S, P> {
    type Metadata = Height;
    type LayoutError = !;

    /*
    #[inline(always)]
    fn try_layout(_: Height) -> Result<Layout, !> {
        Ok(Layout::new::<SumTree<T, S, P, ()>>())
    }
    */

    #[inline(always)]
    fn metadata(this: &Self) -> Height {
        this.height.get()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), height: Height) -> *const Self {
        unsafe {
            mem::transmute(slice::from_raw_parts(thin, height.into()))
        }
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), height: Height) -> *mut Self {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(thin, height.into()))
        }
    }
}

unsafe impl<T, S: MerkleSum<T>, P: Ptr> Pointee for DynInner<T, S, P> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    /*
    #[inline(always)]
    fn try_layout(_: Height) -> Result<Layout, !> {
        Ok(Layout::new::<SumTree<T, S, P, ()>>())
    }
    */

    #[inline(always)]
    fn metadata(this: &Self) -> NonZeroHeight {
        this.height.get().try_into().unwrap()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), height: NonZeroHeight) -> *const Self {
        unsafe {
            mem::transmute(slice::from_raw_parts(thin, height.into()))
        }
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), height: NonZeroHeight) -> *mut Self {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(thin, height.into()))
        }
    }
}

// Drop

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> Drop for SumTree<T, S, P, H> {
    fn drop(&mut self) {
        unsafe {
            match NonZeroHeight::try_from(self.height.get()) {
                Ok(inner_height) => self.tip.dealloc::<DynInner<T, S, P>>(inner_height),
                Err(_) => self.tip.dealloc::<T>(()),
            }
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> Drop for Inner<T, S, P, H> {
    fn drop(&mut self) {
        // SAFETY: left and right have ManuallyDrop wrappers, so they won't be dropped again
        unsafe {
            std::ptr::drop_in_place(self.left_mut());
            std::ptr::drop_in_place(self.right_mut());
        }
    }
}

// Take/Borrow/etc.

unsafe impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Take<DynSumTree<T, S, P>> for SumTree<T, S, P, H> {
    #[inline(always)]
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<DynSumTree<T, S, P>>) -> R
    {
        /*
        let mut this = ManuallyDrop::new(self);
        let this: &mut Inner<T, S, Z, [()]> = (&mut *this).borrow_mut();
        let this: &mut ManuallyDrop<_> = unsafe { &mut *(this as *mut _ as *mut _)};
        f(this)
        */ todo!()
    }
}

unsafe impl<T, S: MerkleSum<T>, P: Ptr> IntoOwned for DynSumTree<T, S, P> {
    type Owned = SumTree<T, S, P>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Borrow<DynSumTree<T, S, P>> for SumTree<T, S, P, H> {
    #[inline(always)]
    fn borrow(&self) -> &DynSumTree<T, S, P> {
        self.as_ref()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> AsRef<DynSumTree<T, S, P>> for SumTree<T, S, P, H> {
    #[inline(always)]
    fn as_ref(&self) -> &DynSumTree<T, S, P> {
        unsafe {
            mem::transmute(slice::from_raw_parts(self as *const _ as *const (), self.height.get().into()))
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> AsMut<DynSumTree<T, S, P>> for SumTree<T, S, P, H> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut DynSumTree<T, S, P> {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(self, self.height.get().into()))
        }
    }
}

unsafe impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Take<DynInner<T, S, P>> for Inner<T, S, P, H> {
    #[inline(always)]
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<DynInner<T, S, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this: &mut DynInner<T, S, P> = (&mut *this).borrow_mut();
        let this: &mut ManuallyDrop<_> = unsafe { &mut *(this as *mut _ as *mut _)};
        f(this)
    }
}

unsafe impl<T, S: MerkleSum<T>, P: Ptr> IntoOwned for DynInner<T, S, P> {
    type Owned = Inner<T, S, P>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Borrow<DynInner<T, S, P>> for Inner<T, S, P, H> {
    #[inline(always)]
    fn borrow(&self) -> &DynInner<T, S, P> {
        self.as_ref()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> BorrowMut<DynInner<T, S, P>> for Inner<T, S, P, H> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut DynInner<T, S, P> {
        self.as_mut()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> AsRef<DynInner<T, S, P>> for Inner<T, S, P, H> {
    #[inline(always)]
    fn as_ref(&self) -> &DynInner<T, S, P> {
        unsafe {
            mem::transmute(slice::from_raw_parts(self as *const _ as *const (), self.height.get().into()))
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> AsMut<DynInner<T, S, P>> for Inner<T, S, P, H> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut DynInner<T, S, P> {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(self, self.height.get().into()))
        }
    }
}

// ---- Load ------
#[derive(Debug, Error)]
#[error("invalid flags")]
pub struct LoadFlagsError(u8);

impl Load for Flags {
    type Error = LoadFlagsError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        /*
        blob.validate_bytes(|blob| {
            match blob[0] {
                0 => Ok(unsafe { blob.assume_valid() }),
                x => Err(LoadFlagsError(x)),
            }
        })*/ todo!()
    }
}

#[derive(Debug, Error)]
#[error("invalid flags")]
pub enum LoadSumTreeError<S: fmt::Debug, P: fmt::Debug, H: fmt::Debug> {
    Flags(LoadFlagsError),
    Sum(S),
    Tip(P),
    Height(H),
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Load for SumTree<T, S, P, H>
where S: Load, P: Load, H: Load,
{
    type Error = LoadSumTreeError<<S as Load>::Error, P::Error, H::Error>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr> Load for DynSumTree<T, S, P>
where S: Load, P: Load,
{
    type Error = LoadSumTreeError<<S as Load>::Error, P::Error, !>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}


#[derive(Debug, Error)]
#[error("invalid flags")]
pub enum LoadInnerError<E: fmt::Debug, H: fmt::Debug> {
    Left(E),
    Right(E),
    Height(H),
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Load for Inner<T, S, P, H>
where S: Load, P: Load, H: Load,
{
    type Error = LoadInnerError<LoadSumTreeError<<S as Load>::Error, P::Error, !>, H::Error>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

// ---- Save ------

pub struct SaveSumTreeState<Q: Ptr, T: Save<Q>, S: MerkleSum<T>, P: Ptr> {
    stack: Vec<*const DynSumTree<T, S, P>>,
    state: TipState<Q, T, S, P>,
}

enum TipState<Q: Ptr, T: Save<Q>, S: MerkleSum<T>, P: Ptr> {
    Ready(*const DynSumTree<T, S, P>),
    PendingLeaf {
        this: *const DynSumTree<T, S, P>,
        leaf: *const T,
        state: T::State,
    },
    Done {
        this: *const DynSumTree<T, S, P>,
        saved_ptr: Q,
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, Q: Ptr> Save<Q> for DynSumTree<T, S, P>
where T: Save<Q>,
      P: Save<Q>,
{
    type State = SaveSumTreeState<Q, T, S, P>;

    fn init_save_state(&self) -> Self::State {
        todo!()
    }

    unsafe fn poll<D: SavePtr<Q>>(&self, state: &mut Self::State, dst: D) -> Result<D, D::Error> {
        todo!()
    }

    unsafe fn encode<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}

// ---- Debug impls ----
impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> fmt::Debug for SumTree<T, S, P, H>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug, H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("SumTree");

        f.field("flags", &self.load_flags(Ordering::Relaxed))
         .field("digest", &self.try_tip_digest())
         .field("sum", &self.try_sum());

        match self.get_dirty_tip() {
            Some(Tip::Inner(inner)) => f.field("tip", &inner),
            Some(Tip::Leaf(leaf)) => f.field("tip", &leaf),
            None => f.field("tip", &self.tip),
        };

        f.field("height", &&self.height)
         .finish()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: ?Sized + GetHeight> fmt::Debug for Inner<T, S, P, H>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug, H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Inner")
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &&self.height)
            .finish()
    }
}

// ---- Clone/ToOwned impls ----

impl<T, S: MerkleSum<T>, P: Ptr> ToOwned for DynSumTree<T, S, P>
where T: Clone, S: Clone, P: Clone,
{
    type Owned = SumTree<T, S, P>;

    fn to_owned(&self) -> Self::Owned {
        todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr> ToOwned for DynInner<T, S, P>
where T: Clone, S: Clone, P: Clone,
{
    type Owned = Inner<T, S, P>;

    fn to_owned(&self) -> Self::Owned {
        let left = self.left().to_owned().strip_height();
        let right = self.right().to_owned().strip_height();
        Inner {
            left: ManuallyDrop::new(left),
            right: ManuallyDrop::new(right),
            height: self.height(),
        }
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Clone for SumTree<T, S, P, H>
where T: Clone, S: Clone, P: Clone, H: Clone
{
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T, S: MerkleSum<T>, P: Ptr, H: GetHeight> Clone for Inner<T, S, P, H>
where T: Clone, S: Clone, P: Clone, H: Clone
{
    fn clone(&self) -> Inner<T, S, P, H> {
        let left = self.left().to_owned().strip_height();
        let right = self.right().to_owned().strip_height();
        Self {
            left: ManuallyDrop::new(left),
            right: ManuallyDrop::new(right),
            height: self.height.clone(),
        }
    }
}

/*
pub type DynInner<T, S, Z> = Inner<T, S, Z, DynNonZeroHeight>;

#[derive(Debug)]
enum TipPtr<T, S: Copy, Z: Zone> {
    Inner(ValidPtr<DynInner<T, S, Z>, Z>),
    Leaf(ValidPtr<T, Z>),
}


impl<T, S: MerkleSum<T>, Z: Zone, H: ?Sized + GetHeight> SumTree<T, S, Z, H>
where T: Decode<Z>,
      S: ValidateBlob,
{
    /// Gets an item in the tree.
    pub fn get<'a>(&'a self, mut idx: usize, zone: &Z) -> Option<&'a T>
        where Z: Get
    {
        if idx >= self.len() {
            return None;
        }

        let mut this: &'a DynSumTree<T, S, Z> = self.as_ref();
        loop {
            match this.get_tip(zone) {
                Tip::Leaf(leaf) => {
                    assert_eq!(idx, 0);
                    break Some(leaf)
                },
                Tip::Inner(inner) if idx < this.len() / 2 => {
                    this = inner.left();
                },
                Tip::Inner(inner) => {
                    idx -= this.len() / 2;
                    this = inner.right();
                }
            }
        }
    }

    fn get_tip<'a>(&'a self, zone: &Z) -> Tip<'a, T, S, Z>
        where Z: Get
    {
        match NonZeroHeight::try_from(self.height.get()) {
            Ok(inner_height) => {
                Tip::Inner(
                    unsafe { zone.get_unchecked(&self.tip, inner_height).this }
                )
            },
            Err(_) => {
                Tip::Leaf(
                    unsafe { zone.get_unchecked(&self.tip, T::make_sized_metadata()).this }
                )
            }
        }
    }
}

impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> SumTree<T, S, Z, H> {


    fn get_tip_ptr(&self) -> TipPtr<T, S, Z>
    {
        if self.height.get().get() == 0 {
            TipPtr::Leaf(unsafe {
                ValidPtr::new_unchecked(FatPtr { raw: self.tip, metadata: () })
            })
        } else {
            let height = NonZeroHeight::try_from(self.height.get()).expect("non-zero height");
            TipPtr::Inner(unsafe {
                ValidPtr::new_unchecked(FatPtr { raw: self.tip, metadata: height })
            })
        }
    }

}

impl<T, S: MerkleSum<T>, Z: Zone, H: GetHeight> SumTree<T,S,Z,H> {
}






impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> SumTree<T, S, Z, H> {

    /// Tries to get the tip digest, if available.
    pub fn try_tip_digest(&self) -> Option<Digest> {
    }
}

impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> SumTree<T, S, Z, H> {
}

impl<T, S: MerkleSum<T>, Z: Zone, H: ?Sized + GetHeight> Inner<T, S, Z, H> {
}

impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> Inner<T, S, Z, H> {
}


impl<T, S: Copy, Z: Zone, H: GetHeight> BorrowMut<DynSumTree<T, S, Z>> for SumTree<T, S, Z, H> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut DynSumTree<T, S, Z> {
        self.as_mut()
    }
}


unsafe impl<T, S: Copy, Z: Zone> IntoOwned for DynInner<T, S, Z> {
    type Owned = Inner<T, S, Z, NonZeroHeight>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        todo!()
    }
}
*/


/*

unsafe impl<T, S: Copy, Z: Zone> Pointee for DynInner<T, S, Z> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    #[inline(always)]
    fn try_layout(_: NonZeroHeight) -> Result<Layout, !> {
        Ok(Layout::new::<Inner<T,S,Z,()>>())
    }

    #[inline(always)]
    fn metadata_from_dropped(dropped: &MaybeDropped<Self>) -> NonZeroHeight {
        let height = unsafe { dropped.get_unchecked().height.get() };
        match NonZeroHeight::try_from(height) {
            Ok(height) => height,
            Err(err) => {
                unreachable!("{:?}", err)
            }
        }
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), height: NonZeroHeight) -> *const Self {
        unsafe {
            mem::transmute(slice::from_raw_parts(thin, height.into()))
        }
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), height: NonZeroHeight) -> *mut Self {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(thin, height.into()))
        }
    }
}


impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> fmt::Debug for Inner<T,S,Z,H>
where S: fmt::Debug,
      H: fmt::Debug,
      T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Inner")
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &&self.height)
            .finish()
    }
}

impl<T, S: Copy, Z: Zone, H: ?Sized + GetHeight> Drop for Inner<T,S,Z,H> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, S, Z: Zone> Verbatim for SumTree<T,S,Z>
where T: Commit,
      S: Commit + MerkleSum<T>,
{
    const LEN: usize = 32 + S::LEN + 1;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write(&self.tip_digest())?
           .write(&self.sum())?
           .write(&self.height())?
           .finish()
    }
}

impl<T, S, Z: Zone> Commit for SumTree<T,S,Z>
where T: Commit,
      S: Commit + MerkleSum<T>,
{
    type Committed = SumTree<T::Committed, S, !>;
}

impl<T, S, Z: Zone, H: ?Sized + GetHeight> Verbatim for Inner<T,S,Z,H>
where T: Commit,
      S: Commit + MerkleSum<T>,
{
    const LEN: usize = (32 + S::LEN) * 2 + 1;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write(&self.left().tip_digest())?
           .write(&self.left().sum())?
           .write(&self.right().tip_digest())?
           .write(&self.right().sum())?
           .write(&self.height().get())?
           .finish()
    }
}

impl<T, S, Z: Zone, H: ?Sized + GetHeight> Commit for Inner<T,S,Z,H>
where T: Commit,
      S: Commit + MerkleSum<T>,
{
    type Committed = Inner<T::Committed, S, !>;
}

#[derive(Debug, Error)]
#[error("invalid inner node blob")]
pub enum ValidateBlobInnerError<S: std::fmt::Debug, H: std::fmt::Debug> {
    Flags(ValidateFlagsError),
    Left(S),
    Right(S),
    Height(H),
}

impl<T, S: Copy, Z: Zone, H: GetHeight> ValidateBlob for Inner<T, S, Z, H>
where S: ValidateBlob,
      H: ValidateBlob,
{
    type Error = ValidateBlobInnerError<<SumTree<T, S, Z, ()> as ValidateBlob>::Error, H::Error>;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.field::<SumTree<T, S, Z, ()>, _>(ValidateBlobInnerError::Left)?;
        blob.field::<SumTree<T, S, Z, ()>, _>(ValidateBlobInnerError::Right)?;
        blob.field::<H, _>(ValidateBlobInnerError::Height)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T, S: Copy, Z: Zone, H: GetHeight> Persist for Inner<T, S, Z, H>
where T: Persist,
      S: 'static + ValidateBlob,
      H: 'static + ValidateBlob,
{
    type Persist = Inner<T::Persist, S, Z::Persist, H>;
    type Error = <Self::Persist as ValidateBlob>::Error;
}

impl<T, S: Copy, Z: Zone> ValidateBlob for DynInner<T, S, Z>
where S: ValidateBlob,
{
    type Error = ValidateBlobInnerError<<SumTree<T, S, Z, ()> as ValidateBlob>::Error, !>;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        let mut blob2 = unsafe { blob.clone().cast_unchecked::<Inner<T, S, Z, ()>>() };
        blob2.field::<SumTree<T, S, Z, ()>, _>(ValidateBlobInnerError::Left)?;
        blob2.field::<SumTree<T, S, Z, ()>, _>(ValidateBlobInnerError::Right)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T, S: Copy, Z: Zone> PersistPointee for DynInner<T, S, Z>
where T: Persist,
      S: 'static + ValidateBlob,
{
    type Persist = DynInner<T::Persist, S, Z::Persist>;
    type Error = <Self::Persist as ValidateBlob>::Error;

    unsafe fn assume_valid(this: &Self::Persist) -> Self::Owned {
        todo!()
    }

    unsafe fn assume_valid_ref(this: &Self::Persist) -> &Self {
        todo!()
    }
}

unsafe impl<'a, T, S: Copy, Z: Zone> ValidatePointeeChildren<'a, Z> for DynInner<T, S, Z>
where T: ValidateChildren<'a, Z>,
      S: 'static + ValidateBlob,
{
    type State = !;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        todo!()
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        todo!()
    }
}

impl<T, S: Copy, Z: Zone> Load<Z> for DynInner<T, S, Z>
where T: Decode<Z>,
      S: 'static + ValidateBlob,
{
}

#[derive(Debug, Error)]
#[error("invalid sum tree blob")]
pub enum ValidateBlobSumTreeError<S: std::fmt::Debug, P: std::fmt::Debug, H: std::fmt::Debug> {
    Flags(ValidateFlagsError),
    Sum(S),
    TipPtr(P),
    Height(H),
}

impl<T, S: Copy, Z: Zone, H: GetHeight> ValidateBlob for SumTree<T, S, Z, H>
where S: ValidateBlob,
      H: ValidateBlob,
{
    type Error = ValidateBlobSumTreeError<S::Error, <Z::PersistPtr as ValidateBlob>::Error, H::Error>;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.field::<Flags, _>(ValidateBlobSumTreeError::Flags)?;
        blob.field::<Digest, _>(|x| x)?;
        blob.field::<S, _>(ValidateBlobSumTreeError::Sum)?;
        blob.field::<Z::PersistPtr, _>(ValidateBlobSumTreeError::TipPtr)?;
        blob.field::<H, _>(ValidateBlobSumTreeError::Height)?;
        unsafe { blob.assume_valid() }
    }
}

impl<T, S: Copy, Z: Zone> ValidateBlob for DynSumTree<T, S, Z>
where S: ValidateBlob,
{
    type Error = ValidateBlobSumTreeError<S::Error, <Z::PersistPtr as ValidateBlob>::Error, !>;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        todo!()
    }
}

unsafe impl<T, S: Copy, Z: Zone, H: GetHeight> Persist for SumTree<T, S, Z, H>
where T: Persist,
      S: 'static + ValidateBlob,
      H: 'static + ValidateBlob,
{
    type Persist = SumTree<T::Persist, S, Z::Persist, H>;
    type Error = <Self::Persist as ValidateBlob>::Error;
}

unsafe impl<T, S: Copy, Z: Zone> PersistPointee for DynSumTree<T, S, Z>
where T: Persist,
      S: 'static + ValidateBlob,
{
    type Persist = DynSumTree<T::Persist, S, Z::Persist>;
    type Error = <Self::Persist as ValidateBlob>::Error;

    unsafe fn assume_valid(this: &Self::Persist) -> SumTree<T, S, Z> {
        todo!()
    }

    unsafe fn assume_valid_ref(this: &Self::Persist) -> &Self {
        mem::transmute(this)
    }
}

#[derive(Debug)]
pub struct ValidateSumTreeState<'a, T, S: Copy, Z: Zone, TState> {
    stack: Vec<&'a DynSumTree<T, S, Z>>,
    leaf: Option<(&'a T, TState)>,
}

unsafe impl<'a, T, S: Copy, Z: Zone> ValidatePointeeChildren<'a, Z> for DynSumTree<T, S, Z>
where T: ValidateChildren<'a, Z>,
      S: 'static + ValidateBlob,
{
    type State = ValidateSumTreeState<'a, T::Persist, S, Z::Persist, T::State>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        ValidateSumTreeState {
            stack: vec![this],
            leaf: None,
        }
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        loop {
            if let Some((leaf, leaf_state)) = state.leaf.as_mut() {
                T::poll(leaf, leaf_state, validator)?;
                state.leaf.take();
            }

            // We don't modify the stack yet, because the validate_ptr() call below might fail; if
            // it fails we have to try again with the same pointer.
            if let Some(tip) = state.stack.last() {
                if let Ok(height) = NonZeroHeight::try_from(tip.height()) {
                    if let Some(inner) = validator.validate_ptr::<DynInner<T, S, Z>>(&tip.tip, height)? {
                        state.stack.pop();
                        state.stack.push(inner.right());
                        state.stack.push(inner.left());
                    }
                } else {
                    if let Some(leaf) = validator.validate_ptr::<T>(&tip.tip, ())? {
                        state.stack.pop();
                        state.leaf = Some((leaf, T::validate_children(leaf)));
                    }
                }
            } else {
                break Ok(())
            }
        }
    }
}

unsafe impl<'a, T, S: Copy, Z: Zone, H: GetHeight> ValidateChildren<'a, Z> for SumTree<T, S, Z, H>
where T: ValidateChildren<'a, Z>,
      S: 'static + ValidateBlob,
      H: 'static + ValidateBlob,
{
    type State = ValidateSumTreeState<'a, T::Persist, S, Z::Persist, T::State>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        <DynSumTree<T, S, Z> as ValidatePointeeChildren<'a, Z>>::validate_children(this.as_ref())
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        <DynSumTree<T, S, Z> as ValidatePointeeChildren<'a, Z>>::poll(this.as_ref(), state, validator)
    }
}


impl<T, S: Copy, Z: Zone> Load<Z> for DynSumTree<T, S, Z>
where T: Decode<Z>,
      S: 'static + ValidateBlob,
{
}

impl<T, S: Copy, Z: Zone> Decode<Z> for SumTree<T, S, Z>
where T: Decode<Z>,
      S: 'static + ValidateBlob,
{
}

impl<T, S: Copy, Z: Zone, Y: Zone> Saved<Y> for DynSumTree<T, S, Z>
where T: Encoded<Y>,
      S: MerkleSum<T>,
{
    type Saved = DynSumTree<T::Encoded, S, Y>;
}

impl<T, S: Copy, Z: Zone, Y: Zone> Encoded<Y> for SumTree<T, S, Z>
where T: Encoded<Y>,
      S: MerkleSum<T>,
{
    type Encoded = SumTree<T::Encoded, S, Y>;
}

pub struct SaveSumTreeState<'a, T: 'a + Encode<'a, Y>, S: Copy, Z: 'a + Zone, Y: Zone> {
    stack: Vec<InnerState<'a, T, S, Z, Y>>,
    state: TipState<'a, T, S, Z, Y>,
}

enum InnerState<'a, T: 'a, S: Copy, Z: 'a + Zone, Y: Zone> {
    Ready(&'a DynSumTree<T, S, Z>),
    DoneLeft {
        tip: &'a DynSumTree<T, S, Z>,
        left_ptr: Y::PersistPtr,
    }
}

enum TipState<'a, T: Encode<'a, Y>, S: Copy, Z: 'a + Zone, Y: Zone> {
    /// Initial state where nothing has been saved.
    Ready(&'a DynSumTree<T, S, Z>),

    /// Leaf node, which needs saving.
    Leaf {
        tip: &'a DynSumTree<T, S, Z>,
        leaf: &'a T,
        leaf_state: T::State,
    },

    /// Inner node whose children have been saved, but the node itself has not.
    Inner {
        tip: &'a DynSumTree<T, S, Z>,
        inner: &'a Inner<T, S, Z, ()>,
        left_ptr: Y::PersistPtr,
        right_ptr: Y::PersistPtr,
    },

    /// Tip body has been saved.
    Done {
        tip: &'a DynSumTree<T, S, Z>,
        tip_ptr: Y::PersistPtr,
    },
}

impl<'a, T, S: Copy, Z: 'a + Zone, Y: Zone> Save<'a, Y> for DynSumTree<T, S, Z>
where T: 'a + Commit + Encode<'a, Y>,
      S: Commit + MerkleSum<T>,
      Z: SavePtr<Y>,
{
    type State = SaveSumTreeState<'a, T, S, Z, Y>;

    fn make_save_state(&'a self) -> Self::State {
        SaveSumTreeState {
            stack: vec![],
            state: TipState::Ready(self),
        }
    }

    fn save_poll<D>(&self, state: &mut Self::State, mut dumper: D) -> Result<(D, D::BlobPtr), D::Error>
        where D: Dumper<Y>
    {
        loop {
            let new_state = match &mut state.state {
                TipState::Ready(tip) => {
                    match tip.get_tip_ptr() {
                        TipPtr::Leaf(leaf_ptr) => {
                            match Z::try_save_ptr(&leaf_ptr, &dumper) {
                                Ok(tip_ptr) => TipState::Done { tip, tip_ptr },
                                Err(leaf) => {
                                    // SAFETY: we do in fact own this leaf value
                                    let leaf: &'a T = unsafe { &*(leaf as *const T) };
                                    TipState::Leaf {
                                        tip,
                                        leaf_state: leaf.make_encode_state(),
                                        leaf,
                                    }
                                }
                            }
                        },
                        TipPtr::Inner(inner_ptr) => {
                            match Z::try_save_ptr(&inner_ptr, &dumper) {
                                Ok(tip_ptr) => TipState::Done { tip, tip_ptr },
                                Err(inner) => {
                                    // SAFETY: we do in fact own this inner value
                                    let inner: &'a DynInner<T, S, Z> = unsafe { &*(inner as *const _) };

                                    state.stack.push(InnerState::Ready(tip));
                                    TipState::Ready(inner.left())
                                }
                            }
                        },
                    }
                },
                TipState::Leaf { tip, leaf, leaf_state } => {
                    let (d, leaf_ptr) = leaf.save_poll(leaf_state, dumper)?;
                    dumper = d;

                    TipState::Done {
                        tip_ptr: D::blob_ptr_to_zone_ptr(leaf_ptr),
                        tip,
                    }
                },
                TipState::Inner { .. } => {
                    todo!()
                }
                TipState::Done { tip, tip_ptr } => {
                    match state.stack.pop() {
                        Some(InnerState::Ready(parent_tip)) => {
                            todo!()
                        },
                        Some(InnerState::DoneLeft { tip: parent_tip, left_ptr } ) => {
                            todo!()
                        },
                        None => {
                            todo!()
                        },
                    }
                },
            };
            state.state = new_state;
        }
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::prelude::*;

    #[test]
    fn new_leaf() {
        let ll = SumTree::<u8, u8, Heap>::new_leaf(0);
        dbg!(&ll);

        let lr = SumTree::<u8, u8, Heap>::new_leaf(1);
        dbg!(&lr);

        let tip_l = ll.try_join(lr).unwrap();
        dbg!(&tip_l);

        assert_eq!(tip_l.sum(), 1);

        /*
        assert_eq!(tip_l.sum(), 1);
        assert_eq!(tip_l.len(), 2);
        tip_l.commit();
        dbg!(&tip_l);

        let ll = SumTree::<u8, u8, _>::new_leaf_in(2, pile);
        let lr = SumTree::<u8, u8, _>::new_leaf_in(3, pile);
        let tip_r = ll.try_join_in(lr, pile).unwrap();
        assert_eq!(tip_r.sum(), 5);
        assert_eq!(tip_r.len(), 2);

        let tip = tip_l.try_join_in(tip_r, pile).unwrap();

        assert_eq!(tip.sum(), 6);
        assert_eq!(tip.len(), 4);
        dbg!(&tip);

        for i in 0 .. 4 {
            assert_eq!(tip.get(i, pile), Some(&(i as u8)));
        }
        assert_eq!(tip.get(4, pile), None);
        */
    }
}
