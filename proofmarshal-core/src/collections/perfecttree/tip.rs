use std::marker::PhantomData;
use std::fmt;
use std::borrow::{Borrow, BorrowMut};
use std::lazy::SyncOnceCell;
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

use super::*;

pub struct Tip<T, S, Z = (), P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToNonZeroHeight = NonZeroHeight> {
    marker: PhantomData<Leaf<T, S, Z, P>>,
    digest: SyncOnceCell<Digest>,
    sum: SyncOnceCell<S>,
    pub(crate) zone: Z,
    ptr: P,
    height: H,
}

pub type TipDyn<T, S, Z, P> = Tip<T, S, Z, P, NonZeroHeightDyn>;

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> fmt::Debug for Tip<T, S, Z, P, H>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ptr = self.try_get_dirty()
                      .map_err(P::from_clean);

        f.debug_struct("Tip")
            .field("digest", &self.digest)
            .field("sum", &self.sum)
            .field("zone", &self.zone)
            .field("ptr", &ptr)
            .field("height", &&self.height)
            .finish()
    }
}

impl<T, S, Z: Zone> Tip<T, S, Z> {
    pub fn new_in(node: InnerNode<T, S, Z>, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        Self::new_unchecked(
            None,
            None,
            zone.borrow_mut().alloc(node),
        )
    }
}

impl<T, S, Z, P: Ptr> Tip<T, S, Z, P> {
    pub(crate) fn new_unchecked(digest: Option<Digest>, sum: Option<S>, node: Bag<InnerNodeDyn<T, S, Z, P>, Z, P>) -> Self {
        let digest = digest.map(SyncOnceCell::from).unwrap_or_default();
        let sum = sum.map(SyncOnceCell::from).unwrap_or_default();
        let (ptr, height, zone) = node.into_raw_parts();
        Self {
            marker: PhantomData,
            digest,
            sum,
            zone,
            ptr,
            height,
        }
    }
}

impl<T, S, Z, P: Ptr, H: ToNonZeroHeight> Tip<T, S, Z, P, H> {
    pub(crate) fn into_raw_parts(self) -> (Option<Digest>, Option<S>, Z, P, H) {
        let mut this = ManuallyDrop::new(self);
        unsafe {
            (this.digest.take(),
             this.sum.take(),
             ptr::read(&this.zone),
             ptr::read(&this.ptr),
             ptr::read(&this.height),
            )
        }
    }

    pub(crate) unsafe fn from_raw_parts(digest: Option<Digest>, sum: Option<S>, zone: Z, ptr: P, height: H) -> Self {
        let digest = digest.map(SyncOnceCell::from).unwrap_or_default();
        let sum = sum.map(SyncOnceCell::from).unwrap_or_default();
        Self {
            marker: PhantomData,
            digest,
            sum,
            zone,
            ptr,
            height,
        }
    }

    pub(crate) fn strip_height(self) -> Tip<T, S, Z, P, DummyNonZeroHeight> {
        let (digest, sum, zone, ptr, _height) = self.into_raw_parts();
        unsafe {
            Tip::from_raw_parts(digest, sum, zone, ptr, DummyNonZeroHeight)
        }
    }
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Tip<T, S, Z, P, H> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn try_get_dirty(&self) -> Result<&InnerNodeDyn<T, S, Z, P>, P::Clean> {
        unsafe {
            self.ptr.try_get_dirty(self.height())
        }
    }
}

pub struct InnerNode<T, S, Z = (), P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToNonZeroHeight = NonZeroHeight> {
    left: ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    right: ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    height: H,
}

pub type InnerNodeDyn<T, S, Z = (), P = <Z as Zone>::Ptr> = InnerNode<T, S, Z, P, NonZeroHeightDyn>;

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> fmt::Debug for InnerNode<T, S, Z, P, H>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      H: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InnerNode")
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &&self.height)
            .finish()
    }
}

impl<T, S, Z, P: Ptr> InnerNode<T, S, Z, P> {
    pub fn try_join(left: SumPerfectTree<T, S, Z, P>, right: SumPerfectTree<T, S, Z, P>)
        -> Result<Self, JoinError<SumPerfectTree<T, S, Z, P>>>
    {
        if left.height() != right.height() {
            Err(JoinError::HeightMismatch { left, right})
        } else if let Some(height) = left.height().try_increment() {
            Ok(Self {
                left: ManuallyDrop::new(left.strip_height()),
                right: ManuallyDrop::new(right.strip_height()),
                height,
            })
        } else {
            Err(JoinError::HeightOverflow { left, right})
        }
    }
}

impl<T, S, Z, P: Ptr, H: ToNonZeroHeight> InnerNode<T, S, Z, P, H> {
    pub(crate) fn strip_height(self) -> InnerNode<T, S, Z, P, DummyNonZeroHeight> {
        todo!()
    }
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> InnerNode<T, S, Z, P, H> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

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
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> InnerNode<T, S, Z, P, H> {
    fn drop(&mut self) {
        if P::NEEDS_DEALLOC {
            unsafe {
                ptr::drop_in_place(self.left_mut());
                ptr::drop_in_place(self.right_mut());
            }
        }
    }
}

impl<T, S, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Tip<T, S, Z, P, H> {
    fn drop(&mut self) {
        if P::NEEDS_DEALLOC {
            let height = self.height.to_nonzero_height();
            unsafe {
                self.ptr.dealloc::<InnerNodeDyn<T, S, Z, P>>(height);
            }
        }
    }
}


// ------- pointee stuff -----------

impl<T, S, Z, P: Ptr> Pointee for InnerNodeDyn<T, S, Z, P> {
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
        let ptr = ptr::slice_from_raw_parts(thin, u8::from(height).into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, u8::from(height).into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Pointee for TipDyn<T, S, Z, P> {
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
        let ptr = ptr::slice_from_raw_parts(thin, u8::from(height).into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, u8::from(height).into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Borrow<InnerNodeDyn<T, S, Z, P>> for InnerNode<T, S, Z, P> {
    fn borrow(&self) -> &InnerNodeDyn<T, S, Z, P> {
        unsafe {
            &*InnerNodeDyn::make_fat_ptr(self as *const _ as *const (), self.height)
        }
    }
}

impl<T, S, Z, P: Ptr> BorrowMut<InnerNodeDyn<T, S, Z, P>> for InnerNode<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerNodeDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerNodeDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
        }
    }
}

unsafe impl<T, S, Z, P: Ptr> Take<InnerNodeDyn<T, S, Z, P>> for InnerNode<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<InnerNodeDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut InnerNodeDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}


// -------- hoard impls -----------

#[derive(Debug, Error)]
#[error("fixme")]
#[doc(hidden)]
pub enum DecodeInnerNodeBytesError<Tree: fmt::Debug, Height: fmt::Debug> {
    Left(Tree),
    Right(Tree),
    Height(Height),
}

impl<T, S, Z, P: PtrBlob, H: ToNonZeroHeight> Blob for InnerNode<T, S, Z, P, H>
where T: Blob,
      S: Blob,
      Z: Blob,
      H: Blob,
{
    const SIZE: usize = <SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::SIZE +
                        <SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::SIZE +
                        <H as Blob>::SIZE;

    type DecodeBytesError = DecodeInnerNodeBytesError<
            <SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::DecodeBytesError,
            <H as Blob>::DecodeBytesError,
        >;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<Self>, <Self as Blob>::DecodeBytesError> {
        todo!()
    }
}

impl<T, S, Z, P: Ptr, H: ToNonZeroHeight> Load for InnerNode<T, S, Z, P, H>
where T: Load,
      S: Blob,
      Z: Zone,
      H: Blob
{
    type Blob = InnerNode<T::Blob, S, (), P::Blob, H>;
    type Zone = Z;

    fn load(_: <Self as Load>::Blob, _: &<Self as Load>::Zone) -> Self {
        todo!()
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeTipBytesError<Sum: fmt::Debug, Zone: fmt::Debug, Ptr: fmt::Debug, Height: fmt::Debug> {
    Sum(Sum),
    Zone(Zone),
    Ptr(Ptr),
    Height(Height),
}

impl<T, S, Z, P: PtrBlob, H: ToNonZeroHeight> Blob for Tip<T, S, Z, P, H>
where T: Blob,
      S: Blob,
      Z: Blob,
      H: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + S::SIZE + Z::SIZE + P::SIZE + H::SIZE;
    type DecodeBytesError = DecodeTipBytesError<S::DecodeBytesError, Z::DecodeBytesError, P::DecodeBytesError, H::DecodeBytesError>;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }
    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<Self>, <Self as Blob>::DecodeBytesError> { todo!() }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test() {
        let left = Leaf::<u8, (), _, _>::new_in(1u8, Heap);
        let right = Leaf::new_in(1u8, Heap);
        let node = InnerNode::try_join(left.into(), right.into()).unwrap();
        let tip = Tip::new_in(node, Heap);
        dbg!(tip);
    }
}
