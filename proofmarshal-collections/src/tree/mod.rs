use std::alloc::Layout;
use std::borrow::{Borrow, BorrowMut};
use std::cell::{Cell, UnsafeCell};
use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit, ManuallyDrop};
use std::num::NonZeroU8;
use std::slice;
use std::sync::atomic::{AtomicU8, Ordering};
use std::ptr;

use owned::{IntoOwned, Take};
use hoard::prelude::*;
use hoard::zone::{ValidPtr, FatPtr, Missing};
use hoard::pointee::{MaybeDropped, Metadata, MetadataKind, Pointee};
use proofmarshal_derive::Prune;

use proofmarshal_core::commit::{Digest, Verbatim, WriteVerbatim};

use crate::fact::{Fact, maybe::Maybe};
use crate::merklesum::MerkleSum;

pub mod height;
use self::height::*;

/// Perfect merkle sum tree.
#[repr(C)]
pub struct SumTree<T, S, Z: Zone = Missing, E = <T as Fact<Z>>::Evidence, H: ?Sized + GetHeight = Height> {
    marker: PhantomData<Leaf<T,Z,E>>,
    flags: AtomicU8,
    tip_digest: UnsafeCell<Digest>,
    sum: UnsafeCell<S>,
    tip: MaybeUninit<Z::Ptr>,
    height: H,
}

pub type Tree<T, Z = Missing, E = <T as Fact<Z>>::Evidence, H = Height> = SumTree<T,(),Z,E,H>;

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone> SumTree<T,S,Z> {
    /// Creates a new leaf from a fact.
    pub fn new_leaf_in(maybe: Maybe<T, Z>, zone: &Z) -> Self
        where Z: Alloc
    {
        let sum = S::from_item(maybe.trust());
        let owned = zone.alloc(Leaf(maybe));
        Self {
            flags: (Flags::HAVE_TIP | Flags::DIGEST_DIRTY).into(),
            marker: PhantomData,
            tip_digest: Digest::default().into(),
            sum: sum.into(),
            tip: MaybeUninit::new(owned.into_inner().into_inner().raw),
            height: Height::new(0).unwrap(),
        }
    }

    pub fn try_join_in(self, right: Self, zone: &Z) -> Result<Self, JoinError<S::Error>>
        where Z: Alloc
    {
        let tip = Inner::new(self, right)?;
        let height = tip.height.into();
        let sum = tip.sum();
        let tip: OwnedPtr<Inner<T,S,Z,T::Evidence,[()]>, Z> = zone.alloc(tip);

        Ok(Self {
            flags: (Flags::HAVE_TIP | Flags::DIGEST_DIRTY).into(),
            marker: PhantomData,
            tip_digest: Digest::default().into(),
            sum: sum.into(),
            tip: MaybeUninit::new(tip.into_inner().into_inner().raw),
            height,
        })
    }
}

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone, H: ?Sized + GetHeight> SumTree<T,S,Z,T::Evidence,H> {
    #[inline]
    pub fn tip_digest(&self) -> Digest
        where T: Verbatim
    {
        if let Some(digest) = self.try_tip_digest() {
            digest
        } else {
            self.fix_dirty_tip_digest()
        }
    }

    fn fix_dirty_tip_digest(&self) -> Digest
        where T: Verbatim
    {
        let digest = match self.get_tip() {
            Some(Ok(leaf_ptr)) => {
                let leaf = Z::try_get_dirty(&leaf_ptr).expect("dirty tip pointer");
                Digest::hash_verbatim(leaf)
            },
            Some(Err(inner_ptr)) => {
                let inner = Z::try_get_dirty(&inner_ptr).expect("dirty tip pointer");
                Digest::hash_verbatim(inner)
            },
            None => unreachable!("tip should be available if digest dirty"),
        };

        // FIXME: actually do atomics here...
        match self.try_lock(Flags::DIGEST_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.tip_digest.get() = digest;
                }

                self.unlock(Flags::DIGEST_LOCKED, Flags::DIGEST_DIRTY);
            },
            Err(old_flags) => {
                todo!("race")
            },
        }
        digest
    }


    #[inline]
    pub fn sum(&self) -> S {
        if let Some(sum) = self.try_sum() {
            sum
        } else {
            self.fix_dirty_sum()
        }
    }

    fn fix_dirty_sum(&self) -> S {
        let sum = match self.get_tip() {
            Some(Ok(leaf_ptr)) => {
                let leaf = Z::try_get_dirty(&leaf_ptr).expect("dirty tip pointer");
                S::from_item(leaf.0.trust())
            },
            Some(Err(inner_ptr)) => {
                let inner = Z::try_get_dirty(&inner_ptr).expect("dirty tip pointer");
                inner.sum()
            },
            None => unreachable!(),
        };

        match self.try_lock(Flags::SUM_LOCKED) {
            Ok(old_flags) => {
                unsafe {
                    *self.sum.get() = sum;
                }

                self.unlock(Flags::SUM_LOCKED, Flags::SUM_DIRTY);
            },
            Err(old_flags) => {
                todo!("race")
            },
        }
        sum
    }

}

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> SumTree<T, S, Z, E, H> {
    /// Gets the height of the tree.
    pub fn height(&self) -> Height {
        self.height.get()
    }

}


#[derive(Debug)]
pub enum JoinError<SumError> {
    HeightMismatch,
    HeightOverflow,
    SumOverflow(SumError),
}

bitflags::bitflags! {
    pub struct Flags: u8 {
        const HAVE_TIP      = 0b00001;
        const DIGEST_DIRTY  = 0b00010;
        const DIGEST_LOCKED = 0b00100;
        const SUM_DIRTY     = 0b01000;
        const SUM_LOCKED    = 0b10000;
    }
}

impl From<Flags> for AtomicU8 {
    #[inline(always)]
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> Drop for SumTree<T, S, Z, E, H> {
    fn drop(&mut self) {
        match self.get_tip() {
            None => {},
            Some(Ok(leaf)) => {
                unsafe { OwnedPtr::new_unchecked(leaf); }
            },
            Some(Err(inner)) => {
                unsafe { OwnedPtr::new_unchecked(inner); }
            },
        }
    }
}

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> fmt::Debug for SumTree<T, S, Z, E, H>
where T: fmt::Debug,
      S: Copy + fmt::Debug,
      E: fmt::Debug,
      H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SumTree")
            .field("flags", &self.load_flags(Ordering::Relaxed))
            .field("digest", &self.try_tip_digest())
            .field("sum", &self.try_sum())
            .field("tip", &self.get_tip())
            .field("height", &&self.height)
            .finish()
    }
}

impl<T, S: Copy, Z: Zone, E, H: ?Sized + GetHeight> SumTree<T, S, Z, E, H> {
    /// Tries to get the sum, if available.
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

    /// Tries to get the tip digest, if available.
    pub fn try_tip_digest(&self) -> Option<Digest> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::DIGEST_DIRTY) {
            None
        } else {
            unsafe { Some(*self.tip_digest.get()) }
        }
    }
}

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> SumTree<T, S, Z, E, H> {
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

    fn get_tip_ptr(&self) -> Option<&Z::Ptr> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::HAVE_TIP) {
            Some(unsafe { &*self.tip.as_ptr() })
        } else {
            None
        }
    }

    fn get_tip(&self) -> Option<Result<ValidPtr<Leaf<T,Z,E>, Z>,
                                       ValidPtr<Inner<T,S,Z,E,[()]>, Z>>>
    {
        self.get_tip_ptr().copied().map(|raw| {
            if self.height.get().get() == 0 {
                Ok(unsafe { ValidPtr::new_unchecked(FatPtr { raw, metadata: () }) })
            } else {
                let height = NonZeroHeight::try_from(self.height.get()).expect("non-zero height");
                Err(unsafe { ValidPtr::new_unchecked(FatPtr { raw, metadata: height }) })
            }
        })
    }

    /// Strips the height.
    fn strip_height(self) -> SumTree<T,S,Z,E,()>
        where H: Sized
    {
        let mut this = ManuallyDrop::new(self);

        // SAFETY: H should be Copy anyway, but easier to just drop it.
        unsafe { ptr::drop_in_place(&mut this.height) };

        // SAFETY: SumTree is #[repr(C)]
        unsafe { mem::transmute_copy::<
            ManuallyDrop<SumTree<T,S,Z,E,H>>,
                         SumTree<T,S,Z,E,()>,
            >(&this)
        }
    }
}


#[derive(Debug)]
pub struct Leaf<T, Z: Zone = Missing, E = <T as Fact<Z>>::Evidence>(Maybe<T, Z, E>);

#[repr(C)]
pub struct Inner<T, S, Z: Zone = Missing, E = <T as Fact<Z>>::Evidence, H: ?Sized + GetHeight = NonZeroHeight> {
    left:  ManuallyDrop<SumTree<T,S,Z,E,()>>,
    right: ManuallyDrop<SumTree<T,S,Z,E,()>>,
    height: H,
}

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone> Inner<T, S, Z, T::Evidence, NonZeroHeight> {
    pub fn new(left: SumTree<T,S,Z>, right: SumTree<T,S,Z>) -> Result<Self, JoinError<S::Error>> {
        if left.height != right.height {
            Err(JoinError::HeightMismatch)
        } else {
            S::try_sum(&left.sum(), &right.sum()).map_err(JoinError::SumOverflow)?;
            match left.height.try_increment() {
                None => Err(JoinError::HeightMismatch),
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

impl<T: Fact<Z>, S: MerkleSum<T>, Z: Zone, H: ?Sized + GetHeight> Inner<T, S, Z, T::Evidence, H> {
    pub fn sum(&self) -> S {
        S::try_sum(&self.left.sum(), &self.right.sum()).expect("sum to be valid")
    }
}

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> Inner<T, S, Z, E, H> {
    pub fn height(&self) -> NonZeroHeight {
        NonZeroHeight::try_from(self.height.get()).expect("inner node to have non-zero height")
    }

    pub fn left(&self) -> &SumTree<T,S,Z,E,[()]> {
        unsafe {
            let next_height = self.height().decrement().into();
            mem::transmute(slice::from_raw_parts(&self.left, next_height))
        }
    }

    pub fn left_mut(&mut self) -> &mut SumTree<T,S,Z,E,[()]> {
        unsafe {
            let next_height = self.height().decrement().into();
            mem::transmute(slice::from_raw_parts_mut(&mut self.left, next_height))
        }
    }

    pub fn right(&self) -> &SumTree<T,S,Z,E,[()]> {
        unsafe {
            let next_height = self.height().decrement().into();
            mem::transmute(slice::from_raw_parts(&self.right, next_height))
        }
    }

    pub fn right_mut(&mut self) -> &mut SumTree<T,S,Z,E,[()]> {
        unsafe {
            let next_height = self.height().decrement().into();
            mem::transmute(slice::from_raw_parts_mut(&mut self.right, next_height))
        }
    }
}

impl<T, S, Z: Zone, E> Borrow<SumTree<T, S, Z, E, [()]>> for SumTree<T, S, Z, E, Height> {
    #[inline(always)]
    fn borrow(&self) -> &SumTree<T, S, Z, E, [()]> {
        unsafe {
            mem::transmute(slice::from_raw_parts(self, self.height.into()))
        }
    }
}

impl<T, S, Z: Zone, E> BorrowMut<SumTree<T, S, Z, E, [()]>> for SumTree<T, S, Z, E, Height> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut SumTree<T, S, Z, E, [()]> {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(self, self.height.into()))
        }
    }
}

unsafe impl<T, S, Z: Zone, E> IntoOwned for Inner<T, S, Z, E, [()]> {
    type Owned = Inner<T, S, Z, E, NonZeroHeight>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        todo!()
    }
}

impl<T, S, Z: Zone, E> Borrow<Inner<T, S, Z, E, [()]>> for Inner<T, S, Z, E, NonZeroHeight> {
    #[inline(always)]
    fn borrow(&self) -> &Inner<T, S, Z, E, [()]> {
        unsafe {
            mem::transmute(slice::from_raw_parts(self, self.height.into()))
        }
    }
}

impl<T, S, Z: Zone, E> BorrowMut<Inner<T, S, Z, E, [()]>> for Inner<T, S, Z, E, NonZeroHeight> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Inner<T, S, Z, E, [()]> {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(self, self.height.into()))
        }
    }
}

unsafe impl<T, S, Z: Zone, E> Take<Inner<T, S, Z, E, [()]>> for Inner<T, S, Z, E, NonZeroHeight> {
    #[inline(always)]
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<Inner<T, S, Z, E, [()]>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this: &mut Inner<T, S, Z, E, [()]> = (&mut *this).borrow_mut();
        let this: &mut ManuallyDrop<_> = unsafe { &mut *(this as *mut _ as *mut _)};
        f(this)
    }
}

unsafe impl<T, S, Z: Zone, E> Pointee for Inner<T, S, Z, E, [()]> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    #[inline(always)]
    fn try_layout(_: NonZeroHeight) -> Result<Layout, !> {
        Ok(Layout::new::<Inner<T,S,Z,E,()>>())
    }

    #[inline(always)]
    fn metadata_from_dropped(dropped: &MaybeDropped<Self>) -> NonZeroHeight {
        let height = unsafe { dropped.get_unchecked().height.len() };
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

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> fmt::Debug for Inner<T,S,Z,E,H>
where S: Copy + fmt::Debug,
      H: fmt::Debug,
      E: fmt::Debug,
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

impl<T, S, Z: Zone, E, H: ?Sized + GetHeight> Drop for Inner<T,S,Z,E,H> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<Z: Zone, T, S: MerkleSum<T>> Fact<Z> for SumTree<T, S>
where T: Fact + Fact<Z> + Verbatim
{
    type Evidence = SumTree<T, S, Z>;

    fn from_evidence(evidence: &Self::Evidence) -> Self {
        Self {
            marker: PhantomData,
            flags: 0.into(),
            tip_digest: evidence.tip_digest().into(),
            sum: evidence.sum().into(),
            tip: MaybeUninit::uninit(),
            height: evidence.height(),
        }
    }
}

impl<T, S, Z: Zone, H: ?Sized + GetHeight> Verbatim for SumTree<T, S, Z, T::Evidence, H>
where T: Fact<Z> + Verbatim,
      S: MerkleSum<T> + Verbatim,
{
    const LEN: usize = <Digest as Verbatim>::LEN + <S as Verbatim>::LEN;

    #[inline(always)]
    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write(&self.tip_digest())?
           .write(&self.sum())?
           .finish()
    }
}

impl<T, S, Z: Zone, H: ?Sized + GetHeight> Verbatim for Inner<T, S, Z, T::Evidence, H>
where T: Fact<Z> + Verbatim,
      S: MerkleSum<T> + Verbatim,
{
    const LEN: usize = (<Digest as Verbatim>::LEN + <S as Verbatim>::LEN) * 2;

    #[inline(always)]
    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write(self.left())?
           .write(self.right())?
           .finish()
    }
}

impl<T, Z: Zone> Verbatim for Leaf<T, Z>
where T: Fact<Z> + Verbatim,
{
    const LEN: usize = <T as Verbatim>::LEN;

    #[inline(always)]
    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        self.0.encode_verbatim(dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::prelude::*;
    use hoard::pile::TryPileMut;

    #[test]
    fn new_leaf() {
        let pile = TryPileMut::default();
        let pile = &pile;

        let l = Maybe::from_fact(Digest::<u8>::default());
        let ll = Tree::new_leaf_in(l, pile);

        let r = Maybe::from_fact(Digest::<u8>::default());
        let lr = Tree::new_leaf_in(r, pile);

        let tip_l = ll.try_join_in(lr, pile).unwrap();


        let l = Maybe::from_fact(Digest::<u8>::default());
        let ll = Tree::new_leaf_in(l, pile);

        let r = Maybe::from_fact(Digest::<u8>::default());
        let lr = Tree::new_leaf_in(r, pile);

        let tip_r = ll.try_join_in(lr, pile).unwrap();

        let tip = tip_l.try_join_in(tip_r, pile).unwrap();
        dbg!(tip.tip_digest());
        dbg!(&tip);
    }
}
