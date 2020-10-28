use std::mem::{self, ManuallyDrop};
use std::borrow::{Borrow, BorrowMut};
use std::lazy::SyncOnceCell;
use std::convert::TryFrom;
use std::ptr;
use std::ops::DerefMut;
use std::marker::PhantomData;
use std::fmt;

use hoard::pointee::Pointee;
use hoard::zone::{Alloc, Get, GetMut, Ptr, Zone};
use hoard::owned::{Take, Own, IntoOwned};
use hoard::bag::Bag;

use crate::collections::perfecttree::height::*;
use crate::collections::perfecttree::{SumPerfectTree, SumPerfectTreeDyn};
use crate::commit::Digest;

use super::length::*;

pub struct SumPeakTree<T, S, Z, P: Ptr = <Z as Zone>::Ptr, L: ?Sized + ToNonZeroLength = NonZeroLength> {
    state: State<T, S, Z, P>,
    len: L,
}

pub type SumPeakTreeDyn<T, S, Z, P = <Z as Zone>::Ptr> = SumPeakTree<T, S, Z, P, NonZeroLengthDyn>;
pub type PeakTree<T, Z, P = <Z as Zone>::Ptr> = SumPeakTree<T, (), Z, P>;

union State<T, S, Z, P: Ptr = <Z as Zone>::Ptr> {
    peak: ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    tip: ManuallyDrop<Inner<T, S, Z, P, DummyInnerLength>>,
}

pub struct Inner<T, S, Z, P: Ptr = <Z as Zone>::Ptr, L: ?Sized + ToInnerLength = InnerLength> {
    marker: PhantomData<T>,
    digest: SyncOnceCell<Digest>,
    sum: SyncOnceCell<S>,
    zone: Z,
    ptr: P,
    len: L,
}

pub type InnerDyn<T, S, Z, P = <Z as Zone>::Ptr> = Inner<T, S, Z, P, InnerLengthDyn>;

pub struct InnerNode<T, S, Z, P: Ptr = <Z as Zone>::Ptr, L: ?Sized + ToInnerLength = InnerLength> {
    left: ManuallyDrop<SumPeakTree<T, S, Z, P, DummyNonZeroLength>>,
    right: ManuallyDrop<SumPeakTree<T, S, Z, P, DummyNonZeroLength>>,
    len: L,
}

pub type InnerNodeDyn<T, S, Z, P = <Z as Zone>::Ptr> = InnerNode<T, S, Z, P, InnerLengthDyn>;

#[derive(Debug)]
pub enum SumPeakTreeKind<Tree, Inner> {
    Peak(Tree),
    Inner(Inner),
}

// ----- debug impls ------
impl<T, S, Z, P: Ptr, L: ?Sized + ToNonZeroLength> fmt::Debug for SumPeakTree<T, S, Z, P, L>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind() {
            SumPeakTreeKind::Peak(peak) => {
                f.debug_tuple("Peak")
                    .field(&peak)
                    .finish()
            },
            SumPeakTreeKind::Inner(tip) => {
                f.debug_tuple("Inner")
                    .field(&tip)
                    .finish()
            },
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> fmt::Debug for Inner<T, S, Z, P, L>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Inner")
            .field("digest", &self.digest)
            .field("sum", &self.sum)
            .field("zone", &self.zone)
            .field("len", &&self.len)
            .finish()
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> fmt::Debug for InnerNode<T, S, Z, P, L>
where T: fmt::Debug,
      S: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
      L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InnerNode")
            .field("left", &self.left())
            .field("left", &self.right())
            .field("len", &&self.len)
            .finish()
    }
}

impl<T, S, Z, P: Ptr> From<SumPerfectTree<T, S, Z, P>> for SumPeakTree<T, S, Z, P> {
    fn from(peak: SumPerfectTree<T, S, Z, P>) -> Self {
        Self {
            len: NonZeroLength::from_height(peak.height()),
            state: State {
                peak: ManuallyDrop::new(peak.strip_height()),
            }
        }
    }
}

impl<T, S, Z, P: Ptr> From<Inner<T, S, Z, P>> for SumPeakTree<T, S, Z, P> {
    fn from(tip: Inner<T, S, Z, P>) -> Self {
        Self {
            len: tip.len().into(),
            state: State {
                tip: ManuallyDrop::new(tip.strip_len()),
            }
        }
    }
}

impl<T, S, Z, P: Ptr> TryFrom<SumPeakTree<T, S, Z, P>> for Inner<T, S, Z, P> {
    type Error = SumPerfectTree<T, S, Z, P>;

    fn try_from(peak_tree: SumPeakTree<T, S, Z, P>) -> Result<Self, Self::Error> {
        match peak_tree.into_kind() {
            SumPeakTreeKind::Inner(inner) => Ok(inner),
            SumPeakTreeKind::Peak(peak) => Err(peak),
        }
    }
}

impl<T, S, Z, P: Ptr> TryFrom<SumPeakTree<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    type Error = Inner<T, S, Z, P>;

    fn try_from(peak_tree: SumPeakTree<T, S, Z, P>) -> Result<Self, Self::Error> {
        match peak_tree.into_kind() {
            SumPeakTreeKind::Peak(peak) => Ok(peak),
            SumPeakTreeKind::Inner(inner) => Err(inner),
        }
    }
}

impl<T, S, Z: Zone> Inner<T, S, Z> {
    pub fn try_join_in(
        left: impl Into<SumPeakTree<T, S, Z>>,
        right: impl Into<SumPeakTree<T, S, Z>>,
        zone: impl BorrowMut<Z>,
    ) -> Result<Result<Self, SumPerfectTree<T, S, Z>>,
                (SumPeakTree<T, S, Z>, SumPeakTree<T, S, Z>)>
        where Z: Alloc
    {
        let left = left.into();
        let right = right.into();

        match InnerNode::try_join(left, right) {
            Ok(Ok(node)) => Ok(Ok(Self::new_in(node, zone))),
            Ok(Err(peak)) => Ok(Err(peak)),
            Err((left, right)) => Err((left, right)),
        }
    }

    pub fn new_in(node: InnerNode<T, S, Z>, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        let node = zone.borrow_mut().alloc(node);
        Self::new_unchecked(None, None, node)
    }

    pub fn new_unchecked(digest: Option<Digest>, sum: Option<S>, node: Bag<InnerNodeDyn<T, S, Z>, Z>) -> Self
        where Z: Alloc
    {
        let (ptr, len, zone) = node.into_raw_parts();

        unsafe {
            Self::from_raw_parts(digest, sum, zone, ptr, len)
        }
    }
}

impl<T, S, Z: Zone> InnerNode<T, S, Z> {
    pub fn try_join(
        left: impl Into<SumPeakTree<T, S, Z>>,
        right: impl Into<SumPeakTree<T, S, Z>>,
    ) -> Result<Result<Self, SumPerfectTree<T, S, Z>>,
                (SumPeakTree<T, S, Z>, SumPeakTree<T, S, Z>)>
        where Z: Alloc
    {
        let left = left.into();
        let right = right.into();

        match left.len().checked_add(right.len()) {
            None => Err((left, right)),
            Some(len) => Ok(
                match len.try_into_inner_length() {
                    Ok(inner_len) => Ok(
                        unsafe {
                            Self::new_unchecked(left.strip_len(), right.strip_len(), inner_len)
                        }
                    ),
                    Err(height) => {
                        let left = SumPerfectTree::try_from(left).ok().unwrap();
                        let right = SumPerfectTree::try_from(right).ok().unwrap();
                        Err(left.try_join(right).ok().unwrap())
                    },
                }
            ),
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToNonZeroLength> SumPeakTree<T, S, Z, P, L> {
    pub fn len(&self) -> NonZeroLength {
        self.len.to_nonzero_length()
    }

    pub fn kind(&self) -> SumPeakTreeKind<&SumPerfectTreeDyn<T, S, Z, P>,
                                          &InnerDyn<T, S, Z, P>>
    {
        match self.len().try_into_inner_length() {
            Ok(inner_len) => SumPeakTreeKind::Inner(
                unsafe {
                    &*InnerDyn::make_fat_ptr(&self.state.tip as *const _ as *const (), inner_len)
                }
            ),
            Err(height) => SumPeakTreeKind::Peak(
                unsafe {
                    &*SumPerfectTree::make_fat_ptr(&self.state.peak as *const _ as *const (), height)
                }
            ),
        }
    }

    pub fn kind_mut(&mut self) -> SumPeakTreeKind<&mut SumPerfectTreeDyn<T, S, Z, P>,
                                                  &mut InnerDyn<T, S, Z, P>>
    {
        match self.len().try_into_inner_length() {
            Ok(inner_len) => SumPeakTreeKind::Inner(
                unsafe {
                    &mut *InnerDyn::make_fat_ptr_mut(&mut self.state.tip as *mut _ as *mut (), inner_len)
                }
            ),
            Err(height) => SumPeakTreeKind::Peak(
                unsafe {
                    &mut *SumPerfectTree::make_fat_ptr_mut(&mut self.state.peak as *mut _ as *mut (), height)
                }
            ),
        }
    }

    pub(crate) fn strip_len(self) -> SumPeakTree<T, S, Z, P, DummyNonZeroLength>
        where L: Sized
    {
        let this = ManuallyDrop::new(self);

        unsafe {
            SumPeakTree {
                state: ptr::read(&this.state),
                len: DummyNonZeroLength,
            }
        }
    }
}

impl<T, S, Z, P: Ptr, L: ToNonZeroLength> SumPeakTree<T, S, Z, P, L> {
    fn into_raw_parts(self) -> (State<T, S, Z, P>, L) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.state),
             ptr::read(&this.len))
        }
    }

    pub fn into_kind(self) -> SumPeakTreeKind<SumPerfectTree<T, S, Z, P>,
                                              Inner<T, S, Z, P>>
    {
        let mut this = ManuallyDrop::new(self);

        match this.kind_mut() {
            SumPeakTreeKind::Peak(peak) => {
                let peak = unsafe { Own::new_unchecked(peak) };
                SumPeakTreeKind::Peak(peak.into_owned())
            },
            SumPeakTreeKind::Inner(inner) => {
                let inner = unsafe { Own::new_unchecked(inner) };
                SumPeakTreeKind::Inner(inner.into_owned())
            },
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> Inner<T, S, Z, P, L> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub(crate) unsafe fn from_raw_parts(digest: Option<Digest>, sum: Option<S>, zone: Z, ptr: P, len: L) -> Self
        where L: Sized
    {
        Self {
            marker: PhantomData,
            digest: digest.map(SyncOnceCell::from).unwrap_or_else(SyncOnceCell::new),
            sum: sum.map(SyncOnceCell::from).unwrap_or_else(SyncOnceCell::new),
            zone,
            ptr,
            len,
        }
    }

    pub(crate) fn into_raw_parts(self) -> (Option<Digest>, Option<S>, Z, P, L)
        where L: Sized
    {
        let mut this = ManuallyDrop::new(self);

        unsafe {
            (this.digest.take(),
             this.sum.take(),
             ptr::read(&this.zone),
             ptr::read(&this.ptr),
             ptr::read(&this.len))
        }
    }

    pub(crate) fn strip_len(self) -> Inner<T, S, Z, P, DummyInnerLength>
        where L: Sized
    {
        let this = ManuallyDrop::new(self);

        unsafe {
            Inner {
                marker: PhantomData,
                digest: ptr::read(&this.digest),
                sum: ptr::read(&this.sum),
                zone: ptr::read(&this.zone),
                ptr: ptr::read(&this.ptr),
                len: DummyInnerLength,
            }
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> InnerNode<T, S, Z, P, L> {
    unsafe fn new_unchecked(
        left: SumPeakTree<T, S, Z, P, DummyNonZeroLength>,
        right: SumPeakTree<T, S, Z, P, DummyNonZeroLength>,
        len: L,
    ) -> Self
        where L: Sized
    {
        Self {
            left: ManuallyDrop::new(left),
            right: ManuallyDrop::new(right),
            len,
        }
    }

    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub fn left(&self) -> &SumPeakTreeDyn<T, S, Z, P> {
        let (left_len, _right_len)= self.len().split();
        unsafe {
            &*SumPeakTreeDyn::make_fat_ptr(&self.left as *const _ as *const _, left_len)
        }
    }

    pub fn left_mut(&mut self) -> &mut SumPeakTreeDyn<T, S, Z, P> {
        let (left_len, _right_len)= self.len().split();
        unsafe {
            &mut *SumPeakTreeDyn::make_fat_ptr_mut(&mut self.left as *mut _ as *mut _, left_len)
        }
    }

    pub fn right(&self) -> &SumPeakTreeDyn<T, S, Z, P> {
        let (_left_len, right_len)= self.len().split();
        unsafe {
            &*SumPeakTreeDyn::make_fat_ptr(&self.right as *const _ as *const _, right_len)
        }
    }

    pub fn right_mut(&mut self) -> &mut SumPeakTreeDyn<T, S, Z, P> {
        let (_left_len, right_len)= self.len().split();
        unsafe {
            &mut *SumPeakTreeDyn::make_fat_ptr_mut(&mut self.right as *mut _ as *mut _, right_len)
        }
    }
}

// --------- drop impls ----------
impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> Drop for InnerNode<T, S, Z, P, L> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToInnerLength> Drop for Inner<T, S, Z, P, L> {
    fn drop(&mut self) {
        unsafe {
            self.ptr.dealloc::<InnerNodeDyn<T, S, Z, P>>(self.len());
        }
    }
}

impl<T, S, Z, P: Ptr, L: ?Sized + ToNonZeroLength> Drop for SumPeakTree<T, S, Z, P, L> {
    fn drop(&mut self) {
        match self.kind_mut() {
            SumPeakTreeKind::Peak(tree) => unsafe { ptr::drop_in_place(tree) },
            SumPeakTreeKind::Inner(tip) => unsafe { ptr::drop_in_place(tip) },
        }
    }
}

// ----- pointee stuff ------
impl<T, S, Z, P: Ptr> Pointee for InnerDyn<T, S, Z, P> {
    type Metadata = InnerLength;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            InnerLength::try_from(len)
                .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Pointee for InnerNodeDyn<T, S, Z, P> {
    type Metadata = InnerLength;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            InnerLength::try_from(len)
                .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Pointee for SumPeakTreeDyn<T, S, Z, P> {
    type Metadata = NonZeroLength;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            NonZeroLength::try_from(len)
                .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, len.into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S, Z, P: Ptr> Borrow<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow(&self) -> &InnerDyn<T, S, Z, P> {
        unsafe {
            &*InnerDyn::make_fat_ptr(self as *const _ as *const (), self.len)
        }
    }
}

impl<T, S, Z, P: Ptr> BorrowMut<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
        }
    }
}

unsafe impl<T, S, Z, P: Ptr> Take<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
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

impl<T, S, Z, P: Ptr> IntoOwned for InnerDyn<T, S, Z, P> {
    type Owned = Inner<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            Inner {
                len: this.len(),
                marker: PhantomData,
                digest: ptr::read(&this.digest),
                sum: ptr::read(&this.sum),
                zone: ptr::read(&this.zone),
                ptr: ptr::read(&this.ptr),
            }
        }
    }
}

impl<T, S, Z, P: Ptr> Borrow<InnerNodeDyn<T, S, Z, P>> for InnerNode<T, S, Z, P> {
    fn borrow(&self) -> &InnerNodeDyn<T, S, Z, P> {
        unsafe {
            &*InnerNodeDyn::make_fat_ptr(self as *const _ as *const (), self.len)
        }
    }
}

impl<T, S, Z, P: Ptr> BorrowMut<InnerNodeDyn<T, S, Z, P>> for InnerNode<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerNodeDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerNodeDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
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

impl<T, S, Z, P: Ptr> Borrow<SumPeakTreeDyn<T, S, Z, P>> for SumPeakTree<T, S, Z, P> {
    fn borrow(&self) -> &SumPeakTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPeakTreeDyn::make_fat_ptr(self as *const _ as *const (), self.len)
        }
    }
}

impl<T, S, Z, P: Ptr> BorrowMut<SumPeakTreeDyn<T, S, Z, P>> for SumPeakTree<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut SumPeakTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPeakTreeDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
        }
    }
}

unsafe impl<T, S, Z, P: Ptr> Take<SumPeakTreeDyn<T, S, Z, P>> for SumPeakTree<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<SumPeakTreeDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut SumPeakTreeDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}
