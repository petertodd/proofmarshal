use std::marker::PhantomData;
use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::mem::{self, ManuallyDrop};
use std::ops::DerefMut;
use std::convert::TryFrom;
use std::ptr;

use thiserror::Error;

use hoard::primitive::Primitive;
use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::load::{MaybeValid, Load, LoadRef};
use hoard::save::{SaveDirty, SaveDirtyPoll, SaveDirtyRef, SaveDirtyRefPoll, BlobSaver};
use hoard::zone::{Alloc, AsZone, Zone, Get, GetMut, Ptr, PtrConst, PtrBlob, FromPtr};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, Own, Ref};
use hoard::bag::Bag;

use crate::collections::merklesum::MerkleSum;
use crate::commit::{Commit, WriteVerbatim, Digest};

pub mod height;
use self::height::*;

mod leaf;
pub use self::leaf::*;

mod tip;
pub use self::tip::*;

pub struct SumPerfectTree<T, S, Z = (), P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToHeight = Height> {
    state: State<T, S, Z, P>,
    height: H,
}

pub type SumPerfectTreeDyn<T, S, Z = (), P = <Z as Zone>::Ptr> = SumPerfectTree<T, S, Z, P, HeightDyn>;

pub type PerfectTree<T, Z = (), P = <Z as Zone>::Ptr> = SumPerfectTree<T, (), Z, P>;
pub type PerfectTreeDyn<T, Z = (), P = <Z as Zone>::Ptr> = SumPerfectTree<T, (), Z, P, HeightDyn>;


#[derive(Debug)]
pub enum Kind<Leaf, Tip> {
    Leaf(Leaf),
    Tip(Tip),
}

union State<T, S, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    leaf: ManuallyDrop<Leaf<T, S, Z, P>>,
    tip: ManuallyDrop<Tip<T, S, Z, P, DummyNonZeroHeight>>,
}

#[derive(Debug)]
pub enum JoinError<Left, Right = Left> {
    HeightMismatch {
        left: Left,
        right: Right,
    },
    HeightOverflow {
        left: Left,
        right: Right,
    },
}

impl<T, S, Z: Zone> SumPerfectTree<T, S, Z> {
    pub fn new_leaf_in(value: T, zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        Leaf::new_in(value, zone).into()
    }

    pub fn try_join(self, right: Self) -> Result<Self, JoinError<Self, Self>>
        where Z: Alloc
    {
        if self.height() != right.height() {
            Err(JoinError::HeightMismatch { left: self, right })
        } else if let Some(_new_height) = self.height().try_increment() {
            let zone = self.zone().clone();
            let node = InnerNode::try_join(self, right)
                                 .ok().expect("error conditions already checked");
            let tip = Tip::new_in(node, zone);
            Ok(Self::from(tip))
        } else {
            Err(JoinError::HeightOverflow { left: self, right })
        }
    }
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToHeight> SumPerfectTree<T, S, Z, P, H> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    fn zone(&self) -> &Z {
        match self.kind() {
            Kind::Tip(tip) => &tip.zone,
            Kind::Leaf(leaf) => leaf.bag.zone(),
        }
    }

    pub fn kind(&self) -> Kind<&Leaf<T, S, Z, P>, &TipDyn<T, S, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            unsafe {
                let tip = &*self.state.tip;
                Kind::Tip(
                    &*TipDyn::make_fat_ptr(tip as *const _ as *const _, height)
                )
            }
        } else {
            unsafe {
                Kind::Leaf(&self.state.leaf)
            }
        }
    }

    pub fn kind_mut(&mut self) -> Kind<&mut Leaf<T, S, Z, P>, &mut TipDyn<T, S, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            unsafe {
                let tip = &mut *self.state.tip;
                Kind::Tip(
                    &mut *TipDyn::make_fat_ptr_mut(tip as *mut _ as *mut _, height)
                )
            }
        } else {
            unsafe {
                Kind::Leaf(&mut self.state.leaf)
            }
        }
    }
}

impl<T, S, Z, P: Ptr, H: ToHeight> SumPerfectTree<T, S, Z, P, H> {
    pub(crate) fn strip_height(self) -> SumPerfectTree<T, S, Z, P, DummyHeight> {
        let this = ManuallyDrop::new(self);
        SumPerfectTree {
            state: unsafe { ptr::read(&this.state) },
            height: DummyHeight,
        }
    }
}

impl<T, S, Z, P: Ptr> SumPerfectTree<T, S, Z, P> {
    pub fn into_kind(self) -> Kind<Leaf<T, S, Z, P>, Tip<T, S, Z, P>> {
        let mut this = ManuallyDrop::new(self);
        match this.kind_mut() {
            Kind::Leaf(leaf) => Kind::Leaf(unsafe { ptr::read(leaf) }),
            Kind::Tip(_) => {
                todo!()
            }
        }
    }
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToHeight> fmt::Debug for SumPerfectTree<T, S, Z, P, H>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

// ------ drop impls ------------

impl<T, S, Z, P: Ptr, H: ?Sized + ToHeight> Drop for SumPerfectTree<T, S, Z, P, H> {
    fn drop(&mut self) {
        match self.kind_mut() {
            Kind::Leaf(leaf) => unsafe { ptr::drop_in_place(leaf) },
            Kind::Tip(tip) => unsafe { ptr::drop_in_place(tip) },
        }
    }
}

impl<T, S, Z, P: Ptr> From<Leaf<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn from(leaf: Leaf<T, S, Z, P>) -> Self {
        Self {
            state: State { leaf: ManuallyDrop::new(leaf) },
            height: Height::new(0).unwrap(),
        }
    }
}

impl<T, S, Z, P: Ptr> From<Tip<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn from(tip: Tip<T, S, Z, P>) -> Self {
        Self {
            height: tip.height().into(),
            state: State {
                tip: ManuallyDrop::new(tip.strip_height())
            },
        }
    }
}

impl<T, S, Z, P: Ptr> Pointee for SumPerfectTreeDyn<T, S, Z, P> {
    type Metadata = Height;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            Height::try_from(len)
                   .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), height: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, height.get().into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, height.get().into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Borrow<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn borrow(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(self as *const _ as *const (), self.height)
        }
    }
}

impl<T, S, Z, P: Ptr> BorrowMut<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
        }
    }
}

unsafe impl<T, S, Z, P: Ptr> Take<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<SumPerfectTreeDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut SumPerfectTreeDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}

impl<T, S, Z, P: Ptr> IntoOwned for SumPerfectTreeDyn<T, S, Z, P> {
    type Owned = SumPerfectTree<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            SumPerfectTree {
                height: this.height(),
                state: ptr::read(&this.state),
            }
        }
    }
}

// ----- hoard impls ----

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeSumPerfectTreeBytesError<
    Leaf: std::error::Error,
    Tip: std::error::Error,
    H: std::error::Error,
> {
    Leaf(Leaf),
    Tip(Tip),
    Height(H),
}

impl<T, S, Z, P: PtrBlob, H: ToHeight> Blob for SumPerfectTree<T, S, Z, P, H>
where T: Blob,
      Z: Blob,
      S: Blob,
      H: Blob,
{
    const SIZE: usize = H::SIZE + <Tip<T, S, Z, P, DummyNonZeroHeight> as Blob>::SIZE;
    type DecodeBytesError = DecodeSumPerfectTreeBytesError<
        <Leaf<T, S, Z, P> as Blob>::DecodeBytesError,
        <Tip<T, S, Z, P, DummyNonZeroHeight> as Blob>::DecodeBytesError,
        H::DecodeBytesError,
    >;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        /*
        dst.write_struct()
           .write_field(&self.tip_digest.get().unwrap())
           .write_field(&self.sum.get().unwrap())
           .write_field(&self.ptr)
           .write_field(&self.zone)
           .write_field(&self.height)
           .done()
        */ todo!()
    }

    fn decode_bytes(src: hoard::blob::Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        /*
        let mut fields = src.struct_fields();

        let tip_digest = fields.trust_field::<Digest>().into_ok();
        let sum = fields.trust_field::<S>().map_err(DecodeSumPerfectTreeBytesError::Sum)?;
        let ptr = fields.trust_field::<P>().map_err(DecodeSumPerfectTreeBytesError::Ptr)?;
        let zone = fields.trust_field::<Z>().map_err(DecodeSumPerfectTreeBytesError::Zone)?;
        let height = fields.trust_field::<H>().map_err(DecodeSumPerfectTreeBytesError::Height)?;
        fields.assert_done();

        Ok(Self {
            marker: PhantomData,
            tip_digest: Some(tip_digest).into(),
            sum: Some(sum).into(),
            ptr,
            zone,
            height,
        }.into())
        */ todo!()
    }
}

impl<T, S, Z, P: Ptr, H: ToHeight> Load for SumPerfectTree<T, S, Z, P, H>
where T: Load,
      S: Blob,
      Z: Zone,
      H: Blob,
{
    type Blob = SumPerfectTree<T::Blob, S, (), P::Blob, H>;
    type Zone = Z;

    fn load(_: <Self as Load>::Blob, _: &<Self as Load>::Zone) -> Self {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test() {
        let l = PerfectTree::new_leaf_in(1u8, Heap);
        let r = PerfectTree::new_leaf_in(2u8, Heap);
        let ll = l.try_join(r).unwrap();

        let l = PerfectTree::new_leaf_in(1u8, Heap);
        let r = PerfectTree::new_leaf_in(2u8, Heap);
        let lr = l.try_join(r).unwrap();

        let tip = ll.try_join(lr).unwrap();
        dbg!(tip);
    }
}

/*
#[derive(Debug)]
pub struct SumPerfectTree<T, S: Copy, Z = (), P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToHeight = Height> {
    marker: PhantomData<T>,
    tip_digest: Cell<Option<Digest<!>>>,
    sum: Cell<Option<S>>,
    ptr: P,
    zone: Z,
    height: H,
}

pub type SumPerfectTreeDyn<T, S, Z, P = <Z as Zone>::Ptr> = SumPerfectTree<T, S, Z, P, DynHeight>;

pub type PerfectTree<T, Z, P = <Z as Zone>::Ptr> = SumPerfectTree<T, (), Z, P>;
pub type PerfectTreeDyn<T, Z, P = <Z as Zone>::Ptr> = SumPerfectTreeDyn<T, (), Z, P>;

#[derive(Debug)]
pub struct Inner<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToNonZeroHeight = NonZeroHeight> {
    left:  ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    right: ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    height: H,
}

pub type InnerDyn<T, S, Z, P = <Z as Zone>::Ptr> = Inner<T, S, Z, P, DynNonZeroHeight>;

#[derive(Debug)]
pub enum Tip<Leaf, Inner> {
    Leaf(Leaf),
    Inner(Inner),
}

#[derive(Debug)]
pub enum TipRef<'a, T, S: Copy, Z, P: Ptr> {
    Leaf(&'a T),
    Inner(&'a InnerDyn<T, S, Z, P>),
}

#[derive(Debug)]
pub enum TipMut<'a, T, S: Copy, Z, P: Ptr> {
    Leaf(&'a mut T),
    Inner(&'a mut InnerDyn<T, S, Z, P>),
}

#[derive(Debug)]
pub enum JoinError<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr> {
    HeightMismatch {
        lhs: SumPerfectTree<T, S, Z, P>,
        rhs: SumPerfectTree<T, S, Z, P>,
    },
    HeightOverflow {
        lhs: SumPerfectTree<T, S, Z, P>,
        rhs: SumPerfectTree<T, S, Z, P>,
    },
}

impl<T, S: Copy, Z: Zone> SumPerfectTree<T, S, Z> {
    pub fn new_leaf_in(value: T, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        let (ptr, (), zone) = zone.borrow_mut().alloc(value).into_raw_parts();

        unsafe {
            Self::from_raw_parts(None, None, zone, ptr, Height::new(0).unwrap())
        }
    }

    pub fn try_join(self, rhs: Self) -> Result<Self, JoinError<T, S, Z>>
        where Z: Alloc
    {
        let mut zone = self.zone;
        Inner::try_join(self, rhs).map(|inner| {
            let inner_bag: Bag<InnerDyn<T, S, Z>, Z> = zone.alloc(inner);
            let (ptr, nonzero_height, _) = inner_bag.into_raw_parts();

            unsafe {
                Self::from_raw_parts(None, None, zone, ptr, nonzero_height.to_height())
            }
        })
    }
}

impl<T, S: MerkleSum<T>, Z, P: Ptr, H: ?Sized + ToHeight> SumPerfectTree<T, S, Z, P, H>
where T: Load,
      Z: Get<P> + AsZone<T::Zone>
{
    pub fn get<'a>(&'a self, idx: usize) -> Result<Option<Ref<'a, T>>, Z::Error> {
        if idx <= self.len() {
            match self.get_tip()? {
                Tip::Leaf(leaf) => {
                    assert_eq!(self.len(), 0);
                    Ok(Some(leaf))
                },
                Tip::Inner(Ref::Borrowed(inner)) => {
                    inner.get(idx)
                },
                Tip::Inner(Ref::Owned(inner)) => {
                    inner.take(idx)
                         .map(|option_t| option_t.map(Ref::Owned))
                }
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Result<Option<&mut T>, Z::Error>
        where Z: GetMut<P>
    {
        let len = self.len();
        if idx <= len {
            match self.get_tip_mut()? {
                Tip::Leaf(leaf) => {
                    assert_eq!(len, 0);
                    Ok(Some(leaf))
                },
                Tip::Inner(inner) => inner.get_mut(idx),
            }
        } else {
            Ok(None)
        }
    }

    pub fn take(self, idx: usize) -> Result<Option<T>, Z::Error>
        where H: Sized,
    {
        let len = self.len();
        if idx <= len {
            match self.take_tip()? {
                Tip::Leaf(leaf) => {
                    assert_eq!(len, 0);
                    Ok(Some(leaf))
                },
                Tip::Inner(inner) => inner.take(idx),
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_tip<'a>(&'a self) -> Result<Tip<Ref<'a, T>, Ref<'a, InnerDyn<T, S, Z, P>>>, Z::Error> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe {
                self.zone.get_unchecked(&self.ptr, height)?
            };
            let inner = inner.trust();
            Ok(Tip::Inner(inner))
        } else {
            let leaf = unsafe {
                self.zone.get_unchecked(&self.ptr, ())?
            };
            let leaf = leaf.trust();
            Ok(Tip::Leaf(leaf))
        }
    }

    pub fn get_tip_mut(&mut self) -> Result<Tip<&mut T, &mut InnerDyn<T, S, Z, P>>, Z::Error>
        where Z: GetMut<P>
    {
        let r = if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe {
                self.zone.get_unchecked_mut(&mut self.ptr, height)?
            };
            let inner = inner.trust();
            Tip::Inner(inner)
        } else {
            let leaf = unsafe {
                self.zone.get_unchecked_mut(&mut self.ptr, ())?
            };
            let leaf = leaf.trust();
            Tip::Leaf(leaf)
        };

        self.sum = None.into();
        self.tip_digest = None.into();

        Ok(r)
    }

    pub fn take_tip(self) -> Result<Tip<T, Inner<T, S, Z, P>>, Z::Error>
        where H: Sized,
    {
        let (_tip_digest, _sum, zone, ptr, height) = self.into_raw_parts();

        if let Ok(height) = NonZeroHeight::try_from(height.to_height()) {
            let inner = unsafe {
                zone.take_unchecked::<InnerDyn<T, S, Z, P>>(ptr, height)?
            };
            let inner = inner.trust();
            Ok(Tip::Inner(inner))
        } else {
            let leaf = unsafe {
                zone.take_unchecked::<T>(ptr, ())?
            };
            let leaf = leaf.trust();
            Ok(Tip::Leaf(leaf))
        }
    }
}

impl<T, S: MerkleSum<T>, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Inner<T, S, Z, P, H>
where T: Load,
      Z: Get<P> + AsZone<T::Zone>
{
    fn get<'a>(&'a self, idx: usize) -> Result<Option<Ref<'a, T>>, Z::Error> {
        let len = self.height().to_height().len();
        if idx < len / 2 {
            self.left.get(idx)
        } else if idx < len {
            self.right.get(idx - (len / 2))
        } else {
            Ok(None)
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Result<Option<&mut T>, Z::Error>
        where Z: GetMut<P>
    {
        let len = self.height().to_height().len();
        if idx < len / 2 {
            self.left.get_mut(idx)
        } else if idx < len {
            self.right.get_mut(idx - (len / 2))
        } else {
            Ok(None)
        }
    }

    pub fn take(self, idx: usize) -> Result<Option<T>, Z::Error>
        where H: Sized
    {
        let (left, right, height) = self.into_raw_parts();
        let len = height.to_height().len();
        if idx < len / 2 {
            left.take(idx)
        } else if idx < len {
            right.take(idx - (len / 2))
        } else {
            Ok(None)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ToHeight> SumPerfectTree<T, S, Z, P, H> {
    pub unsafe fn from_raw_parts(
        tip_digest: Option<Digest>,
        sum: Option<S>,
        zone: Z,
        ptr: P,
        height: H,
    ) -> Self
    {
        Self {
            marker: PhantomData,
            tip_digest: tip_digest.into(),
            sum: sum.into(),
            zone,
            ptr,
            height,
        }
    }

    pub fn blob_from_raw_parts(
        tip_digest: Option<Digest>,
        sum: Option<S>,
        zone: Z,
        ptr: P,
        height: H,
    ) -> Self
        where S: Blob,
              Z: Blob,
              P: PtrBlob,
    {
        unsafe {
            Self::from_raw_parts(tip_digest, sum, zone, ptr, height)
        }
    }

    pub fn into_raw_parts(self) -> (Option<Digest>, Option<S>, Z, P, H) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.tip_digest).into_inner(),
             ptr::read(&this.sum).into_inner(),
             ptr::read(&this.zone),
             ptr::read(&this.ptr),
             ptr::read(&this.height))
        }
    }

    fn strip(self) -> SumPerfectTree<T, S, Z, P, DummyHeight> {
        let this = ManuallyDrop::new(self);

        unsafe {
            SumPerfectTree {
                marker: PhantomData,
                tip_digest: ptr::read(&this.tip_digest),
                sum: ptr::read(&this.sum),
                ptr: ptr::read(&this.ptr),
                zone: ptr::read(&this.zone),
                height: DummyHeight,
             }
        }
    }
}


impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> SumPerfectTree<T, S, Z, P, H> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    pub fn len(&self) -> usize {
        self.height().len()
    }

    pub fn sum(&self) -> S
        where S: MerkleSum<T>
    {
        if let Some(sum) = self.sum.get() {
            sum
        } else {
            let sum = match self.try_get_dirty_tip() {
                Ok(TipRef::Leaf(leaf)) => S::from_item(leaf),
                Ok(TipRef::Inner(inner)) => inner.sum(),
                Err(_clean) => unreachable!(),
            };

            self.sum.set(Some(sum));
            sum
        }
    }

    pub fn tip_digest(&self) -> Digest
        where S: MerkleSum<T>,
              T: Commit
    {
        if let Some(tip_digest) = self.tip_digest.get() {
            tip_digest
        } else {
            let tip_digest = match self.try_get_dirty_tip() {
                Ok(TipRef::Leaf(leaf)) => leaf.commit().cast(),
                Ok(TipRef::Inner(inner)) => inner.commit().cast(),
                Err(_clean) => unreachable!(),
            };

            self.tip_digest.set(Some(tip_digest));
            tip_digest
        }
    }

    pub fn try_get_dirty_tip<'a>(&'a self) -> Result<TipRef<'a, T, S, Z, P>, P::Clean> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe { self.ptr.try_get_dirty(height)? };
            Ok(TipRef::Inner(inner))
        } else {
            let leaf = unsafe { self.ptr.try_get_dirty(())? };
            Ok(TipRef::Leaf(leaf))
        }
    }

    pub fn try_get_dirty_tip_mut<'a>(&'a mut self) -> Result<TipMut<'a, T, S, Z, P>, P::Clean> {
        let r = if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe { self.ptr.try_get_dirty_mut(height)? };
            TipMut::Inner(inner)
        } else {
            let leaf = unsafe { self.ptr.try_get_dirty_mut(())? };
            TipMut::Leaf(leaf)
        };

        self.sum = None.into();
        self.tip_digest = None.into();

        Ok(r)
    }
}

impl<T, S: Copy, Z: Zone> Inner<T, S, Z> {
    pub fn try_join(lhs: SumPerfectTree<T, S, Z>, rhs: SumPerfectTree<T, S, Z>) -> Result<Self, JoinError<T, S, Z>> {
        if lhs.height() != rhs.height() {
            Err(JoinError::HeightMismatch { lhs, rhs })
        } else if let Some(height) = lhs.height().try_increment() {
            unsafe {
                Ok(Self::new_unchecked(lhs, rhs, height))
            }
        } else {
            Err(JoinError::HeightOverflow { lhs, rhs })
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ToNonZeroHeight> Inner<T, S, Z, P, H> {
    unsafe fn new_unchecked<HL: ToHeight, HR: ToHeight>(
        left: SumPerfectTree<T, S, Z, P, HL>,
        right: SumPerfectTree<T, S, Z, P, HR>,
        height: H,
    ) -> Self {
        Self {
            left: ManuallyDrop::new(left.strip()),
            right: ManuallyDrop::new(right.strip()),
            height,
        }
    }

    fn new_blob(
        left: SumPerfectTree<T, S, Z, P, DummyHeight>,
        right: SumPerfectTree<T, S, Z, P, DummyHeight>,
        height: H,
    ) -> Self
        where Z: Blob,
              P: PtrBlob,
    {
        unsafe {
            Self::new_unchecked(left, right, height)
        }
    }


    fn into_raw_parts(self) -> (SumPerfectTree<T, S, Z, P, DummyHeight>, SumPerfectTree<T, S, Z, P, DummyHeight>, H) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&*this.left),
             ptr::read(&*this.right),
             ptr::read(&this.height))
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Inner<T, S, Z, P, H> {
    pub fn left(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(&self.left as *const _ as *const _, self.height().decrement())
        }
    }

    pub fn left_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(&mut self.left as *mut _ as *mut _, self.height().decrement())
        }
    }

    pub fn right(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(&self.right as *const _ as *const _, self.height().decrement())
        }
    }

    pub fn right_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(&mut self.right as *mut _ as *mut _, self.height().decrement())
        }
    }

    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn sum(&self) -> S
        where S: MerkleSum<T>
    {
        S::sum(self.left().sum(), self.right().sum())
    }
}

// ---- drop impls -----
impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> Drop for SumPerfectTree<T, S, Z, P, H> {
    fn drop(&mut self) {
        if P::NEEDS_DEALLOC {
            if let Ok(height) = NonZeroHeight::try_from(self.height()) {
                unsafe { self.ptr.dealloc::<InnerDyn<T, S, Z, P>>(height) }
            } else {
                unsafe { self.ptr.dealloc::<T>(()) }
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Drop for Inner<T, S, Z, P, H> {
    fn drop(&mut self) {
        if P::NEEDS_DEALLOC {
            unsafe {
                ptr::drop_in_place(self.left_mut());
                ptr::drop_in_place(self.right_mut());
            }
        }
    }
}


// ------ unsized type stuff ------


unsafe impl<T, S: Copy, Z, P: Ptr> Take<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<SumPerfectTreeDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut SumPerfectTreeDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> IntoOwned for SumPerfectTreeDyn<T, S, Z, P> {
    type Owned = SumPerfectTree<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            SumPerfectTree {
                height: this.height.to_height(),
                marker: PhantomData,
                tip_digest: ptr::read(&this.tip_digest),
                sum: ptr::read(&this.sum),
                zone: ptr::read(&this.zone),
                ptr: ptr::read(&this.ptr),
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> Pointee for InnerDyn<T, S, Z, P> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            NonZeroHeight::try_from(len)
                          .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), height: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, height.to_height().get().into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, height.to_height().get().into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S: Copy, Z, P: Ptr> Borrow<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow(&self) -> &InnerDyn<T, S, Z, P> {
        unsafe {
            &*InnerDyn::make_fat_ptr(self as *const _ as *const (), self.height)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> BorrowMut<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
        }
    }
}

unsafe impl<T, S: Copy, Z, P: Ptr> Take<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<InnerDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut InnerDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> IntoOwned for InnerDyn<T, S, Z, P> {
    type Owned = Inner<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            Inner {
                height: this.height.to_nonzero_height(),
                left: ptr::read(&this.left),
                right: ptr::read(&this.right),
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> Commit for SumPerfectTree<T, S, Z, P, H>
where T: Commit,
      S: MerkleSum<T>,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + S::VERBATIM_LEN + 1;
    type Committed = SumPerfectTree<T::Committed, S>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.tip_digest().as_bytes());
        dst.write(&self.sum());
        dst.write(&self.height());
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Commit for Inner<T, S, Z, P, H>
where T: Commit,
      S: MerkleSum<T>,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + S::VERBATIM_LEN + 1;
    type Committed = SumPerfectTree<T::Committed, S>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.left().tip_digest().as_bytes());
        dst.write(&self.left().sum());
        dst.write(&self.right().tip_digest().as_bytes());
        dst.write(&self.right().sum());
        dst.write(&self.height());
    }
}


// ------ hoard ------

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeSumPerfectTreeBytesError<
    S: std::error::Error,
    Z: std::error::Error,
    P: std::error::Error,
    H: std::error::Error,
> {
    Sum(S),
    Zone(Z),
    Ptr(P),
    Height(H),
}

impl<T: 'static, S: Copy, Z, P: PtrBlob, H: ToHeight> Blob for SumPerfectTree<T, S, Z, P, H>
where Z: Blob,
      S: Blob,
      H: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + S::SIZE + P::SIZE + Z::SIZE + H::SIZE;
    type DecodeBytesError = DecodeSumPerfectTreeBytesError<S::DecodeBytesError, Z::DecodeBytesError, P::DecodeBytesError, H::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.tip_digest.get().unwrap())
           .write_field(&self.sum.get().unwrap())
           .write_field(&self.ptr)
           .write_field(&self.zone)
           .write_field(&self.height)
           .done()
    }

    fn decode_bytes(src: hoard::blob::Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();

        let tip_digest = fields.trust_field::<Digest>().into_ok();
        let sum = fields.trust_field::<S>().map_err(DecodeSumPerfectTreeBytesError::Sum)?;
        let ptr = fields.trust_field::<P>().map_err(DecodeSumPerfectTreeBytesError::Ptr)?;
        let zone = fields.trust_field::<Z>().map_err(DecodeSumPerfectTreeBytesError::Zone)?;
        let height = fields.trust_field::<H>().map_err(DecodeSumPerfectTreeBytesError::Height)?;
        fields.assert_done();

        Ok(Self {
            marker: PhantomData,
            tip_digest: Some(tip_digest).into(),
            sum: Some(sum).into(),
            ptr,
            zone,
            height,
        }.into())
    }
}

unsafe impl<T: 'static, S: Copy, Z, P: PtrBlob> BlobDyn for SumPerfectTreeDyn<T, S, Z, P>
where S: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeSumPerfectTreeBytesError<S::DecodeBytesError, Z::DecodeBytesError, P::DecodeBytesError, !>;

    fn try_size(_: <Self as Pointee>::Metadata) -> Result<usize, <Self as Pointee>::LayoutError> {
        Ok(<SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.tip_digest.get().unwrap())
           .write_field(&self.sum.get().unwrap())
           .write_field(&self.ptr)
           .write_field(&self.zone)
           .done()
    }

    fn decode_bytes(_: Bytes<'_, Self>) -> Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> {
        todo!()
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeInnerBytesError<
    S: std::error::Error,
    Z: std::error::Error,
    P: std::error::Error,
    H: std::error::Error,
> {
    Left(DecodeSumPerfectTreeBytesError<S, Z, P, !>),
    Right(DecodeSumPerfectTreeBytesError<S, Z, P, !>),
    Height(H),
}

impl<T: 'static, S: Copy, Z, P: PtrBlob, H: ToNonZeroHeight> Blob for Inner<T, S, Z, P, H>
where Z: Blob,
      S: Blob,
      H: Blob,
{
    const SIZE: usize = <SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::SIZE * 2 + H::SIZE;
    type DecodeBytesError = DecodeInnerBytesError<S::DecodeBytesError, Z::DecodeBytesError, P::DecodeBytesError, H::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.left)
           .write_field(&*self.right)
           .write_field(&self.height)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();

        let left = fields.trust_field().map_err(DecodeInnerBytesError::Left)?;
        let right = fields.trust_field().map_err(DecodeInnerBytesError::Right)?;
        let height = fields.trust_field().map_err(DecodeInnerBytesError::Height)?;
        fields.assert_done();

        unsafe {
            Ok(Inner::new_unchecked::<DummyHeight, DummyHeight>(left, right, height).into())
        }
    }
}

unsafe impl<T: 'static, S: Copy, Z, P: PtrBlob> BlobDyn for InnerDyn<T, S, Z, P>
where S: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeInnerBytesError<S::DecodeBytesError, Z::DecodeBytesError, P::DecodeBytesError, !>;

    fn try_size(_: <Self as Pointee>::Metadata) -> Result<usize, <Self as Pointee>::LayoutError> {
        Ok(<Inner<T, S, Z, P, DummyNonZeroHeight> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.left)
           .write_field(&*self.right)
           .done()
    }

    fn decode_bytes(_: Bytes<'_, Self>) -> Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> {
        todo!()
    }
}

impl<T:, S: Copy, Z, P: Ptr, H: ToHeight> Load for SumPerfectTree<T, S, Z, P, H>
where T: Load,
      S: Blob,
      Z: Zone,
      H: Blob,
{
    type Blob = SumPerfectTree<T::Blob, S, (), P::Blob, H>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        let (tip_digest, sum, (), ptr, height) = blob.into_raw_parts();
        Self {
            marker: PhantomData,
            tip_digest: tip_digest.into(),
            sum: sum.into(),
            ptr: P::from_clean(P::Clean::from_blob(ptr)),
            zone: *zone,
            height,
        }
    }
}

impl<T:, S: Copy, Z, P: Ptr, H: ToNonZeroHeight> Load for Inner<T, S, Z, P, H>
where T: Load,
      S: Blob,
      Z: Zone,
      H: Blob,
{
    type Blob = Inner<T::Blob, S, (), P::Blob, H>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        let (left, right, height) = blob.into_raw_parts();
        unsafe {
            Self::new_unchecked(Load::load(left, zone), Load::load(right, zone), height)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> LoadRef for SumPerfectTreeDyn<T, S, Z, P>
where T: Load,
      Z: Zone,
      S: Blob,
{
    type BlobDyn = SumPerfectTreeDyn<T::Blob, S, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(blob: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = BlobDyn::decode_bytes(blob)?.trust();

        Ok(MaybeValid::from(Ref::Owned(
            SumPerfectTree {
                marker: PhantomData,
                tip_digest: blob.tip_digest.clone(),
                sum: blob.sum.clone(),
                ptr: P::from_clean(P::Clean::from_blob(blob.ptr)),
                zone: *zone,
                height: blob.height,
            }
        )))
    }
}

impl<T, S: Copy, Z, P: Ptr> LoadRef for InnerDyn<T, S, Z, P>
where T: Load,
      S: Blob,
      Z: Zone,
{
    type BlobDyn = InnerDyn<T::Blob, S, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(blob: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = BlobDyn::decode_bytes(blob)?.trust();
        let (left, right, height) = blob.into_raw_parts();

        let this = unsafe {
            Inner::new_unchecked(Load::load(left, zone), Load::load(right, zone), height)
        };

        Ok(MaybeValid::from(Ref::Owned(this)))
    }
}

// ------ Save impl -----------

// The actual save machinery:
struct TreeSaveDirtyImpl<T: SaveDirty, S, P: PtrConst> {
    tip_digest: Digest,
    sum: S,
    state: TreeSaveDirtyState<T, S, P>,
}

enum TreeSaveDirtyState<T: SaveDirty, S, P: PtrConst> {
    DirtyLeaf(Box<T::SaveDirtyPoll>),
    DirtyInner(Box<InnerSaveDirtyImpl<T, S, P>>),
    Done(P::Blob),
}

struct InnerSaveDirtyImpl<T: SaveDirty, S, P: PtrConst> {
    left: TreeSaveDirtyImpl<T, S, P>,
    right: TreeSaveDirtyImpl<T, S, P>,
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> SumPerfectTree<T, S, Z, P, H>
where T: Commit + SaveDirty,
      T::CleanPtr: FromPtr<P::Clean>,
      S: MerkleSum<T>,
{
    fn make_tree_save_dirty_impl(&self) -> TreeSaveDirtyImpl<T, S, P::Clean> {
        TreeSaveDirtyImpl {
            tip_digest: self.tip_digest(),
            sum: self.sum(),
            state: match self.try_get_dirty_tip() {
                Ok(TipRef::Leaf(leaf)) => {
                    TreeSaveDirtyState::DirtyLeaf(Box::new(leaf.init_save_dirty()))
                },
                Ok(TipRef::Inner(inner)) => {
                    TreeSaveDirtyState::DirtyInner(Box::new(inner.make_inner_save_dirty_impl()))
                },
                Err(clean_ptr) => TreeSaveDirtyState::Done(clean_ptr.to_blob()),
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Inner<T, S, Z, P, H>
where T: Commit + SaveDirty,
      T::CleanPtr: FromPtr<P::Clean>,
      S: MerkleSum<T>,
{
    fn make_inner_save_dirty_impl(&self) -> InnerSaveDirtyImpl<T, S, P::Clean> {
        InnerSaveDirtyImpl {
            left: self.left().make_tree_save_dirty_impl(),
            right: self.right().make_tree_save_dirty_impl(),
        }
    }
}

impl<T: SaveDirty, S, P: PtrConst> TreeSaveDirtyImpl<T, S, P>
where S: Copy + Blob,
      T::CleanPtr: FromPtr<P>,
{
    fn poll<B>(&mut self, saver: &mut B, height: Height) -> Result<(), B::Error>
        where B: BlobSaver<CleanPtr = P>
    {
        let blob_ptr = match &mut self.state {
            TreeSaveDirtyState::DirtyLeaf(leaf) => {
                leaf.save_dirty_poll(saver)?;
                saver.save_blob(&leaf.encode_blob())?
            },
            TreeSaveDirtyState::DirtyInner(inner) => {
                let height = NonZeroHeight::try_from(height).expect("invalid height");
                inner.poll(saver, height)?;

                let inner_blob = inner.encode_blob(DummyNonZeroHeight);
                saver.save_blob(&inner_blob)?
            },
            TreeSaveDirtyState::Done(_) => {
                return Ok(())
            }
        };
        self.state = TreeSaveDirtyState::Done(blob_ptr);
        Ok(())
    }

    fn encode_blob<H: ToHeight>(&self, height: H) -> SumPerfectTree<T::Blob, S, (), P::Blob, H> {
        if let TreeSaveDirtyState::Done(ptr_blob) = self.state {
            SumPerfectTree::blob_from_raw_parts(
                Some(self.tip_digest),
                Some(self.sum),
                (),
                ptr_blob,
                height,
            )
        } else {
            panic!()
        }
    }
}

impl<T: SaveDirty, S, P: PtrConst> InnerSaveDirtyImpl<T, S, P>
where S: Copy + Blob,
      T::CleanPtr: FromPtr<P>,
{
    fn poll<B>(&mut self, saver: &mut B, height: NonZeroHeight) -> Result<(), B::Error>
        where B: BlobSaver<CleanPtr = P>
    {
        let child_height = height.decrement();
        self.left.poll(saver, child_height)?;
        self.right.poll(saver, child_height)
    }

    fn encode_blob<H: ToNonZeroHeight>(&self, height: H) -> Inner<T::Blob, S, (), P::Blob, H> {
        Inner::new_blob(
            self.left.encode_blob(DummyHeight),
            self.right.encode_blob(DummyHeight),
            height,
        )
    }
}

pub struct SumPerfectTreeSaveDirty<T: SaveDirty, S, P: PtrConst, H> {
    height: H,
    inner: TreeSaveDirtyImpl<T, S, P>,
}

impl<T: SaveDirty, S: Copy, P: PtrConst, H: ToHeight> SaveDirtyPoll for SumPerfectTreeSaveDirty<T, S, P, H>
where S: MerkleSum<T>,
      H: Primitive,
      T::CleanPtr: FromPtr<P>
{
    type CleanPtr = P;
    type SavedBlob = SumPerfectTree<T::Blob, S, (), P::Blob, H>;

    fn save_dirty_poll_impl<B>(&mut self, saver: &mut B) -> Result<(), B::Error>
        where B: BlobSaver<CleanPtr = P>
    {
        self.inner.poll(saver, self.height.to_height())
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        self.inner.encode_blob(self.height)
    }
}

impl<T:, S: Copy, Z, P: Ptr, H: ToHeight> SaveDirty for SumPerfectTree<T, S, Z, P, H>
where T: Commit + SaveDirty,
      S: MerkleSum<T>,
      Z: Zone,
      H: Primitive,
      T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveDirtyPoll = SumPerfectTreeSaveDirty<T, S, P::Clean, H>;

    fn init_save_dirty(&self) -> <Self as SaveDirty>::SaveDirtyPoll {
        SumPerfectTreeSaveDirty {
            height: self.height,
            inner: self.make_tree_save_dirty_impl(),
        }
    }
}

pub struct SumPerfectTreeDynSaveDirty<T: SaveDirty, S, P: PtrConst> {
    height: Height,
    inner: TreeSaveDirtyImpl<T, S, P>,
}

impl<T: SaveDirty, S: Copy, P: PtrConst> SaveDirtyRefPoll for SumPerfectTreeDynSaveDirty<T, S, P>
where S: MerkleSum<T>,
      T::CleanPtr: FromPtr<P>
{
    type CleanPtr = P;
    type SavedBlobDyn = SumPerfectTreeDyn<T::Blob, S, (), P::Blob>;

    fn blob_metadata(&self) -> Height {
        self.height
    }

    fn save_dirty_ref_poll_impl<B>(&mut self, saver: &mut B) -> Result<(), B::Error>
        where B: BlobSaver<CleanPtr = P>
    {
        self.inner.poll(saver, self.height)
    }

    fn encode_blob_dyn_bytes<'a>(&self, _dst: BytesUninit<'a, Self::SavedBlobDyn>) -> Bytes<'a, Self::SavedBlobDyn> {
        todo!()
    }
}

impl<T:, S: Copy, Z, P: Ptr> SaveDirtyRef for SumPerfectTreeDyn<T, S, Z, P>
where T: Commit + SaveDirty,
      S: MerkleSum<T>,
      Z: Zone,
      T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveDirtyRefPoll = SumPerfectTreeDynSaveDirty<T, S, P::Clean>;

    fn init_save_dirty_ref(&self) -> Self::SaveDirtyRefPoll {
        SumPerfectTreeDynSaveDirty {
            height: self.height.to_height(),
            inner: self.make_tree_save_dirty_impl(),
        }
    }
}

pub struct InnerDynSaveDirty<T: SaveDirty, S, P: PtrConst> {
    height: NonZeroHeight,
    inner: InnerSaveDirtyImpl<T, S, P>,
}

impl<T: SaveDirty, S: Copy, P: PtrConst> SaveDirtyRefPoll for InnerDynSaveDirty<T, S, P>
where S: MerkleSum<T>,
      T::CleanPtr: FromPtr<P>
{
    type CleanPtr = P;
    type SavedBlobDyn = InnerDyn<T::Blob, S, (), P::Blob>;

    fn blob_metadata(&self) -> NonZeroHeight {
        self.height
    }

    fn save_dirty_ref_poll_impl<B>(&mut self, saver: &mut B) -> Result<(), B::Error>
        where B: BlobSaver<CleanPtr = P>
    {
        self.inner.poll(saver, self.height)
    }

    fn encode_blob_dyn_bytes<'a>(&self, _dst: BytesUninit<'a, Self::SavedBlobDyn>) -> Bytes<'a, Self::SavedBlobDyn> {
        todo!()
    }
}

impl<T, S: Copy, Z, P: Ptr> SaveDirtyRef for InnerDyn<T, S, Z, P>
where T: Commit + SaveDirty,
      Z: Zone,
      S: MerkleSum<T>,
      T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveDirtyRefPoll = InnerDynSaveDirty<T, S, P::Clean>;

    fn init_save_dirty_ref(&self) -> Self::SaveDirtyRefPoll {
        InnerDynSaveDirty {
            height: self.height(),
            inner: self.make_inner_save_dirty_impl(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;
    use hoard::pile::{PileMut, VecSaver, Offset};

    #[test]
    fn test() {
        let ll = PerfectTree::new_leaf_in(1u8, Heap);
        assert_eq!(ll.height().get(), 0);

        let lr = PerfectTree::new_leaf_in(2u8, Heap);
        assert_eq!(lr.height().get(), 0);

        let tip = ll.try_join(lr).unwrap();
        let _ = dbg!(tip.try_get_dirty_tip());

        dbg!(tip.commit());
        let _ = dbg!(tip.try_get_dirty_tip());
        dbg!(tip);
    }

    #[test]
    fn test_save() {
        let pile = PileMut::<[u8]>::default();

        let ll = PerfectTree::new_leaf_in(42u8, pile);
        assert_eq!(ll.height().get(), 0);

        assert_eq!(VecSaver::new(pile.into()).save_dirty(&ll),
            (vec![
                 42,
                 42,0,0,0,0,0,0,0,0,0,0,0,0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
             ],
             Offset::new(1)),
        );

        let lr = PerfectTree::new_leaf_in(43u8, pile);

        let tip = ll.try_join(lr).unwrap();
        assert_eq!(VecSaver::new(pile.into()).save_dirty(&tip),
            (vec![
                 42,
                 43,
                 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  0, 0, 0, 0, 0, 0, 0, 0,
                 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  1, 0, 0, 0, 0, 0, 0, 0,
                 144, 166, 31, 71, 11, 60, 188, 148, 181, 232, 180, 157, 94, 143, 94, 219, 159, 97, 255, 207, 94, 51, 109, 15, 214, 181, 46, 53, 44, 173, 99, 39,
                 2, 0, 0, 0, 0, 0, 0, 0,
                 1
             ],
             Offset::new(82)),
        );
    }
}
*/
