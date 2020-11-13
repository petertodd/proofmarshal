use std::marker::PhantomData;
use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::error;
use std::mem::{self, ManuallyDrop};
use std::ops::{Deref, DerefMut};
use std::convert::TryFrom;
use std::ptr;

use thiserror::Error;

use hoard::primitive::Primitive;
use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::load::{MaybeValid, Load, LoadRef};
use hoard::save::{Save, SavePoll, SaveRef, SaveRefPoll, Saver};
use hoard::ptr::{AsZone, Zone, Get, GetMut, Ptr, PtrClean, PtrBlob};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, RefOwn, Ref};
use hoard::bag::Bag;

use crate::commit::{Commit, Digest, HashCommit, Sha256Digest};
use crate::unreachable_unchecked;

use crate::collections::{
    height::*,
    length::*,
    raw,
    perfecttree::{PerfectTree, PerfectTreeDyn, PerfectTreeDynSavePoll},
};

#[repr(C)]
pub struct Pair<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, P, D>>,
    len: InnerLength,
}

#[repr(C)]
pub struct PairDyn<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, P, D>>,
    len: InnerLengthDyn,
}

#[repr(C)]
pub struct Inner<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P, D>>,
    len: InnerLength,
}

#[repr(C)]
pub struct InnerDyn<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P, D>>,
    len: InnerLengthDyn,
}

#[repr(C)]
pub struct PeakTree<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P, D>>,
    len: NonZeroLength,
}

#[repr(C)]
pub struct PeakTreeDyn<T, P: Ptr, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P, D>>,
    len: NonZeroLengthDyn,
}

#[derive(Debug)]
pub enum Kind<Peak, Inner> {
    Peak(Peak),
    Inner(Inner),
}

impl<T, P: Ptr, D: Digest> From<PerfectTree<T, P, D>> for PeakTree<T, P, D> {
    fn from(peak: PerfectTree<T, P, D>) -> Self {
        let len = NonZeroLength::from_height(peak.height());
        let raw = peak.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, P: Ptr, D: Digest> From<Inner<T, P, D>> for PeakTree<T, P, D> {
    fn from(inner: Inner<T, P, D>) -> Self {
        let len = inner.len().into();
        let raw = inner.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTree<T, P, D>
where T: Load
{
    pub(crate) fn try_push_peak(self, peak: PerfectTree<T, P, D>) -> Result<Self, (Self, PerfectTree<T, P, D>)>
        where P: Default + GetMut
    {
        match self.into_kind() {
            Kind::Inner(inner) => {
                inner.try_push_peak(peak)
                     .map_err(|(inner, peak)| (inner.into(), peak))
            },
            Kind::Peak(left) if left.height() == peak.height() => {
                PerfectTree::try_join(left, peak)
                            .map_err(|(left, right)| (left.into(), right))
                            .map(Self::from)
            },
            Kind::Peak(left) => {
                Inner::try_join_peaks(left, peak)
                      .map_err(|(inner, peak)| (Self::from(inner), peak))
                      .map(Self::from)
            }
        }
    }

    fn merge_peak(self, right: PerfectTree<T, P, D>) -> PerfectTree<T, P, D>
        where P: Default + GetMut
    {
        match self.into_kind() {
            Kind::Inner(inner) => inner.merge_peak(right),
            Kind::Peak(left) => PerfectTree::try_join(left, right).ok().expect("overflow"),
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTree<T, P, D> {
    pub fn into_kind(self) -> Kind<PerfectTree<T, P, D>, Inner<T, P, D>> {
        let len = self.len();
        let node = self.into_raw_node();

        match len.try_into_inner_length() {
            Ok(len) => {
                let inner = unsafe { Inner::from_raw_node(node, len) };
                Kind::Inner(inner)
            },
            Err(height) => {
                let peak = unsafe { PerfectTree::from_raw_node(node, height) };
                Kind::Peak(peak)
            }
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTree<T, P, D>
where T: Load
{
    pub fn into_get(self, height: Height) -> Option<PerfectTree<T, P, D>>
        where P: Get
    {
        match self.into_kind() {
            Kind::Peak(peak) if peak.height() == height => Some(peak),
            Kind::Peak(_) => None,
            Kind::Inner(inner) => inner.into_get(height),
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTreeDyn<T, P, D>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, P, D>>>
        where P: Get
    {
        match self.kind() {
            Kind::Peak(peak) if peak.height() == height => Some(Ref::Borrowed(peak)),
            Kind::Peak(_) => None,
            Kind::Inner(inner) => inner.get(height),
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTreeDyn<T, P, D> {
    pub fn len(&self) -> NonZeroLength {
        self.len.to_nonzero_length()
    }

    pub fn kind(&self) -> Kind<&PerfectTreeDyn<T, P, D>, &InnerDyn<T, P, D>> {
        match self.len().try_into_inner_length() {
            Ok(len) => {
                let inner = unsafe { InnerDyn::from_raw_node_ref(&self.raw, len) };
                Kind::Inner(inner)
            },
            Err(height) => {
                let peak = unsafe { PerfectTreeDyn::from_raw_node_ref(&self.raw, height) };
                Kind::Peak(peak)
            }
        }
    }

    pub fn kind_mut(&mut self) -> Kind<&mut PerfectTreeDyn<T, P, D>, &mut InnerDyn<T, P, D>> {
        match self.len().try_into_inner_length() {
            Ok(len) => {
                let inner = unsafe { InnerDyn::from_raw_node_mut(&mut self.raw, len) };
                Kind::Inner(inner)
            },
            Err(height) => {
                let peak = unsafe { PerfectTreeDyn::from_raw_node_mut(&mut self.raw, height) };
                Kind::Peak(peak)
            }
        }
    }
}

impl<T, P: Ptr, D: Digest> Inner<T, P, D> {
    pub fn try_join_peaks(left: PerfectTree<T, P, D>, right: PerfectTree<T, P, D>)
        -> Result<Self, (PerfectTree<T, P, D>, PerfectTree<T, P, D>)>
        where P: Default
    {
        let pair = Pair::try_join_peaks(left, right)?;
        Ok(Self::new(pair))
    }

    pub fn new(pair: Pair<T, P, D>) -> Self
        where P: Default
    {
        Self::new_unchecked(None, P::alloc(pair))
    }

    pub fn new_unchecked(digest: Option<D>, pair: Bag<PairDyn<T, P, D>, P>) -> Self {
        let (ptr, len) = pair.into_raw_parts();
        let raw = raw::Node::new(digest, ptr);

        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, P: Ptr, D: Digest> Inner<T, P, D>
where T: Load
{
    fn merge_peak(self, right: PerfectTree<T, P, D>) -> PerfectTree<T, P, D>
        where P: Default + GetMut
    {
        let pair = self.into_pair();
        pair.merge_peak(right)
    }

    pub(crate) fn try_push_peak(self, peak: PerfectTree<T, P, D>) -> Result<PeakTree<T, P, D>, (Self, PerfectTree<T, P, D>)>
        where P: Default + GetMut
    {
        match self.len().push_peak(peak.height()) {
            Ok(new_len) => {
                let (new_left_len, _new_right_len) = new_len.split();
                //eprintln!("new_left_len = 0b{:b}, new_right_len = 0b{:b}", new_left_len, new_right_len);

                if new_left_len == self.len() {
                    Ok(Self::new(Pair::new(self.into(), peak.into())).into())
                } else {
                    //eprintln!("self.len() = 0b{:b}, peak.len() = 0b{:b}", self.len(), peak.len());
                    let (old_left, old_right) = self.into_pair().into_split();
                    //eprintln!("old_left.len() = 0b{:b}, old_right.len() = 0b{:b}", old_left.len(), old_right.len());

                    let new_right = old_right.try_push_peak(peak).ok().expect("overflow already checked");

                    match new_right.into_kind() {
                        Kind::Inner(new_right) => {
                            Ok(Self::new(Pair::new(old_left, new_right.into())).into())
                        },
                        Kind::Peak(new_right) => {
                            //eprintln!("merging 0b{:b} with 0b{:b}", old_left.len(), new_right.len());
                            Ok(old_left.try_push_peak(new_right)
                                       .ok().expect("overflow already checked"))
                        }
                    }

                }
            },
            Err(Some(_height)) => Ok(self.merge_peak(peak).into()),
            Err(None) => {
                Err((self, peak))
            }
        }
    }

    pub fn into_get(self, height: Height) -> Option<PerfectTree<T, P, D>>
        where P: Get
    {
        if self.len().contains(height) {
            self.into_pair().into_get(height)
        } else {
            None
        }
    }

    pub fn into_pair(self) -> Pair<T, P, D>
        where P: Get
    {
        let len = self.len();
        let raw = self.into_raw_node();
        unsafe {
            raw.take::<PairDyn<T, P, D>>(len)
               .trust()
        }
    }
}

impl<T, P: Ptr, D: Digest> InnerDyn<T, P, D>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, P, D>>>
        where P: Get
    {
        if self.len().contains(height) {
            match self.get_pair() {
                Ref::Borrowed(pair) => pair.get(height),
                Ref::Owned(pair) => pair.into_get(height)
                                        .map(Ref::Owned)
            }
        } else {
            None
        }
    }

    pub fn get_pair(&self) -> Ref<PairDyn<T, P, D>>
        where P: Get
    {
        unsafe {
            self.raw.get::<PairDyn<T, P, D>>(self.len())
                    .trust()
        }
    }

    pub fn get_pair_mut(&mut self) -> &mut PairDyn<T, P, D>
        where P: GetMut
    {
        let len = self.len();
        unsafe {
            self.raw.get_mut::<PairDyn<T, P, D>>(len)
                    .trust()
        }
    }
}

impl<T, P: Ptr, D: Digest> InnerDyn<T, P, D> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub fn pair_commit(&self) -> HashCommit<Pair<T::Commitment, (), D>, D>
        where T: Commit
    {
        self.try_pair_commit()
            .unwrap_or_else(|| self.calc_pair_commit())
    }

    fn calc_pair_commit(&self) -> HashCommit<Pair<T::Commitment, (), D>, D>
        where T: Commit
    {
        let pair = self.try_get_dirty_pair()
                       .ok().expect("digest missing yet inner ptr clean");
        let commit = HashCommit::new(pair);
        self.raw.set_digest(commit.digest());
        commit
    }

    pub fn try_pair_commit(&self) -> Option<HashCommit<Pair<T::Commitment, (), D>, D>>
        where T: Commit
    {
        self.raw.digest().map(HashCommit::from_digest)
    }
}

impl<T, P: Ptr, D: Digest> Pair<T, P, D>
where T: Load
{
    fn merge_peak(self, peak: PerfectTree<T, P, D>) -> PerfectTree<T, P, D>
        where P: Default + GetMut
    {
        let (left, right) = self.into_split();
        let peak = right.merge_peak(peak);
        left.merge_peak(peak)
    }
}


impl<T, P: Ptr, D: Digest> Pair<T, P, D>
{
    pub fn try_join_peaks(left: PerfectTree<T, P, D>, right: PerfectTree<T, P, D>)
        -> Result<Self, (PerfectTree<T, P, D>, PerfectTree<T, P, D>)>
    {
        if left.height() > right.height() {
            let expected_len = (1 << left.height().get()) | (1 << right.height().get());

            let r = Self::new(left.into(), right.into());
            assert_eq!(r.len(), expected_len);
            Ok(r)
        } else {
            Err((left, right))
        }
    }

    pub fn new(left: PeakTree<T, P, D>, right: PeakTree<T, P, D>) -> Self {
        assert!(left.len().min_height() > right.len().max_height());
        let len = left.len().get() | right.len().get();
        let len = InnerLength::try_from(len.get()).unwrap();

        assert_eq!(len.split(), (left.len(), right.len()));

        unsafe {
            Self::new_unchecked(left.into_raw_node(), right.into_raw_node(), len)
        }
    }

    pub unsafe fn new_unchecked(left: raw::Node<T, P, D>, right: raw::Node<T, P, D>, len: InnerLength) -> Self {
        Self::from_raw_pair(
            raw::Pair { left, right },
            len
        )
    }

    pub fn into_split(self) -> (PeakTree<T, P, D>, PeakTree<T, P, D>) {
        let (left_len, right_len) = self.len().split();
        let raw = self.into_raw_pair();

        unsafe {
            (PeakTree::from_raw_node(raw.left, left_len),
             PeakTree::from_raw_node(raw.right, right_len))
        }
    }
}

impl<T, P: Ptr, D: Digest> Pair<T, P, D>
where T: Load
{
    pub fn into_get(self, height: Height) -> Option<PerfectTree<T, P, D>>
        where P: Get
    {
        if self.len().contains(height) {
            let (left, right) = self.into_split();
            if left.len().contains(height) {
                left.into_get(height)
            } else {
                right.into_get(height)
            }
        } else {
            None
        }
    }
}

impl<T, P: Ptr, D: Digest> PairDyn<T, P, D>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, P, D>>>
        where P: Get
    {
        if self.len().contains(height) {
            let (left, right) = self.split();
            if left.len().contains(height) {
                left.get(height)
            } else {
                right.get(height)
            }
        } else {
            None
        }
    }
}

impl<T, P: Ptr, D: Digest> PairDyn<T, P, D> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub fn split(&self) -> (&PeakTreeDyn<T, P, D>, &PeakTreeDyn<T, P, D>) {
        let (left_len, right_len) = self.len().split();
        unsafe {
            (PeakTreeDyn::from_raw_node_ref(&self.raw.left, left_len),
             PeakTreeDyn::from_raw_node_ref(&self.raw.right, right_len))
        }
    }

    pub fn split_mut(&mut self) -> (&mut PeakTreeDyn<T, P, D>, &mut PeakTreeDyn<T, P, D>) {
        let (left_len, right_len) = self.len().split();
        let (left, right) = self.raw.split_mut();
        unsafe {
            (PeakTreeDyn::from_raw_node_mut(left, left_len),
             PeakTreeDyn::from_raw_node_mut(right, right_len))
        }
    }

    pub fn left(&self) -> &PeakTreeDyn<T, P, D> {
        self.split().0
    }

    pub fn left_mut(&mut self) -> &mut PeakTreeDyn<T, P, D> {
        self.split_mut().0
    }

    pub fn right(&self) -> &PeakTreeDyn<T, P, D> {
        self.split().1
    }

    pub fn right_mut(&mut self) -> &mut PeakTreeDyn<T, P, D> {
        self.split_mut().1
    }

}

// --------- conversions from raw -------------

impl<T, P: Ptr, D: Digest> Pair<T, P, D> {
    pub unsafe fn from_raw_pair(raw: raw::Pair<T, P, D>, len: InnerLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_pair(self) -> raw::Pair<T, P, D> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr, D: Digest> PairDyn<T, P, D> {
    pub unsafe fn from_raw_pair_ref(raw: &raw::Pair<T, P, D>, len: InnerLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_pair_mut(raw: &mut raw::Pair<T, P, D>, len: InnerLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
    }
}

impl<T, P: Ptr, D: Digest> Inner<T, P, D> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, P, D>, len: InnerLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, P, D> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr, D: Digest> InnerDyn<T, P, D> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, P, D>, len: InnerLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, P, D>, len: InnerLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
    }

    pub fn try_get_dirty_pair(&self) -> Result<&PairDyn<T, P, D>, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(self.len())
                    .map(MaybeValid::trust)
        }
    }
}

impl<T, P: Ptr, D: Digest> PeakTree<T, P, D> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, P, D>, len: NonZeroLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, P, D> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr, D: Digest> PeakTreeDyn<T, P, D> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, P, D>, len: NonZeroLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, P, D>, len: NonZeroLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
    }
}

// ------- pointee impls ----------

macro_rules! impl_pointee {
    ($t:ident, $meta_ty:ty) => {
        impl<T, P: Ptr, D: Digest> Pointee for $t<T, P, D> {
            type Metadata = $meta_ty;
            type LayoutError = !;

            fn metadata(ptr: *const Self) -> Self::Metadata {
                unsafe {
                    let ptr: *const [()] = mem::transmute(ptr);
                    let len: usize = ptr.len();

                    <$meta_ty>::try_from(len)
                               .unwrap_or_else(|_|
                                   unreachable_unchecked!("invalid metadata")
                               )
                }
            }

            fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const Self {
                let len = len.get();
                let ptr = ptr::slice_from_raw_parts(thin, len.into());
                unsafe { mem::transmute(ptr) }
            }

            fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
                let len = len.get();
                let ptr = ptr::slice_from_raw_parts_mut(thin, len.into());
                unsafe { mem::transmute(ptr) }
            }
        }
    }
}

impl_pointee!(PeakTreeDyn, NonZeroLength);
impl_pointee!(InnerDyn, InnerLength);
impl_pointee!(PairDyn, InnerLength);

// --------- deref impls ----------

macro_rules! impl_deref {
    ($t:ident => $u:ident) => {
        impl<T, P: Ptr, D: Digest> Borrow<$u<T, P, D>> for $t<T, P, D> {
            fn borrow(&self) -> &$u<T, P, D> {
                unsafe {
                    &*$u::make_fat_ptr(self as *const _ as *const (), self.len)
                }
            }
        }

        impl<T, P: Ptr, D: Digest> BorrowMut<$u<T, P, D>> for $t<T, P, D> {
            fn borrow_mut(&mut self) -> &mut $u<T, P, D> {
                unsafe {
                    &mut *$u::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
                }
            }
        }

        unsafe impl<T, P: Ptr, D: Digest> Take<$u<T, P, D>> for $t<T, P, D> {
            fn take_unsized<F, R>(self, f: F) -> R
                where F: FnOnce(RefOwn<$u<T, P, D>>) -> R
            {
                let mut this = ManuallyDrop::new(self);
                let this_dyn: &mut $u<T, P, D> = this.deref_mut().borrow_mut();

                unsafe {
                    f(RefOwn::new_unchecked(this_dyn))
                }
            }
        }

        impl<T, P: Ptr, D: Digest> IntoOwned for $u<T, P, D> {
            type Owned = $t<T, P, D>;

            fn into_owned(self: RefOwn<'_, Self>) -> Self::Owned {
                let this = RefOwn::leak(self);
                unsafe {
                    $t {
                        marker: PhantomData,
                        len: this.len(),
                        raw: ptr::read(&this.raw),
                    }
                }
            }
        }

        impl<T, P: Ptr, D: Digest> Deref for $t<T, P, D> {
            type Target = $u<T, P, D>;

            fn deref(&self) -> &Self::Target {
                self.borrow()
            }
        }

        impl<T, P: Ptr, D: Digest> DerefMut for $t<T, P, D> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.borrow_mut()
            }
        }
    }
}

impl_deref!(PeakTree => PeakTreeDyn);
impl_deref!(Inner => InnerDyn);
impl_deref!(Pair => PairDyn);

// ------- commit impls ---------
impl<T, P: Ptr, D: Digest> Commit for PairDyn<T, P, D>
where T: Commit
{
    type Commitment = Pair<T::Commitment, (), D>;

    fn to_commitment(&self) -> Self::Commitment {
        let left = self.left().to_commitment();
        let right = self.right().to_commitment();

        Pair::new(left, right)
    }
}

impl<T, P: Ptr, D: Digest> Commit for InnerDyn<T, P, D>
where T: Commit
{
    type Commitment = Inner<T::Commitment, (), D>;

    fn to_commitment(&self) -> Self::Commitment {
        let digest = self.pair_commit().digest();
        let raw = raw::Node::new(Some(digest), ());
        unsafe { Inner::from_raw_node(raw, self.len()) }
    }
}

impl<T, P: Ptr, D: Digest> Commit for PeakTreeDyn<T, P, D>
where T: Commit
{
    type Commitment = PeakTree<T::Commitment, (), D>;

    fn to_commitment(&self) -> Self::Commitment {
        match self.kind() {
            Kind::Peak(peak) => peak.to_commitment().into(),
            Kind::Inner(inner) => inner.to_commitment().into(),
        }
    }
}

macro_rules! impl_commit_for_sized {
    ($( $t:ident, )+) => {$(
        impl<T, P: Ptr, D: Digest> Commit for $t<T, P, D>
        where T: Commit
        {
            type Commitment = $t<T::Commitment, (), D>;

            fn to_commitment(&self) -> Self::Commitment {
                self.deref().to_commitment()
            }
        }
    )+}
}

impl_commit_for_sized! {
    Pair, Inner, PeakTree,
}


// ------- hoard impls ----------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePeakTreeBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, P: Ptr, D: Digest> Blob for PeakTree<T, P, D>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, P, D> as Blob>::SIZE + <NonZeroLength as Blob>::SIZE;
    type DecodeBytesError = DecodePeakTreeBytesError<<raw::Node<T, P, D> as Blob>::DecodeBytesError, <NonZeroLength as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.len)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let len = fields.trust_field().map_err(Self::DecodeBytesError::NonZeroLength)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_node(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> Load for PeakTree<T, P, D>
where T: Load
{
    type Blob = PeakTree<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let len = blob.len;
        let raw = blob.into_raw_node();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_node(raw, len) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodePeakTreeDynBytesError<Raw: error::Error>(pub(crate) Raw);

unsafe impl<T, P: Ptr, D: Digest> BlobDyn for PeakTreeDyn<T, P, D>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodePeakTreeDynBytesError<<raw::Node<T, P, D> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<raw::Node<T, P, D> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let len = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodePeakTreeDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { PeakTree::from_raw_node(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> LoadRef for PeakTreeDyn<T, P, D>
where T: Load
{
    type BlobDyn = PeakTreeDyn<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_owned_from_bytes(src: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = PeakTree::<T, P, D>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(owned))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeInnerBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, P: Ptr, D: Digest> Blob for Inner<T, P, D>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, P, D> as Blob>::SIZE + <InnerLength as Blob>::SIZE;
    type DecodeBytesError = DecodeInnerBytesError<<raw::Node<T, P, D> as Blob>::DecodeBytesError, <InnerLength as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.len)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let len = fields.trust_field().map_err(Self::DecodeBytesError::NonZeroLength)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_node(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> Load for Inner<T, P, D>
where T: Load
{
    type Blob = Inner<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let len = blob.len;
        let raw = blob.into_raw_node();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_node(raw, len) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodeInnerDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, P: Ptr, D: Digest> BlobDyn for InnerDyn<T, P, D>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodeInnerDynBytesError<<raw::Node<T, P, D> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<raw::Node<T, P, D> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let len = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodeInnerDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { Inner::from_raw_node(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> LoadRef for InnerDyn<T, P, D>
where T: Load
{
    type BlobDyn = InnerDyn<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_owned_from_bytes(src: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = Inner::<T, P, D>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(owned))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePairBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, P: Ptr, D: Digest> Blob for Pair<T, P, D>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Pair<T, P, D> as Blob>::SIZE + <InnerLength as Blob>::SIZE;
    type DecodeBytesError = DecodePairBytesError<<raw::Pair<T, P, D> as Blob>::DecodeBytesError, <InnerLength as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.len)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let len = fields.trust_field().map_err(Self::DecodeBytesError::NonZeroLength)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_pair(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> Load for Pair<T, P, D>
where T: Load
{
    type Blob = Pair<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let len = blob.len;
        let raw = blob.into_raw_pair();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_pair(raw, len) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodePairDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, P: Ptr, D: Digest> BlobDyn for PairDyn<T, P, D>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodePairDynBytesError<<raw::Pair<T, P, D> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<raw::Pair<T, P, D> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let len = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodePairDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { Pair::from_raw_pair(raw, len) }.into())
    }
}

impl<T, P: Ptr, D: Digest> LoadRef for PairDyn<T, P, D>
where T: Load
{
    type BlobDyn = PairDyn<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_owned_from_bytes(src: Bytes<'_, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Self::Owned>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;

        let owned = Pair::<T, P, D>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(owned))
    }
}

// -------- save impls ------------

#[doc(hidden)]
pub enum PeakTreeDynSavePoll<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> {
    Peak(Box<PerfectTreeDynSavePoll<Q, T, P, D>>),
    Inner(Box<InnerDynSavePoll<Q, T, P, D>>),
}

#[doc(hidden)]
pub struct PeakTreeSavePoll<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest>(
    PeakTreeDynSavePoll<Q, T, P, D>
);

#[doc(hidden)]
pub struct InnerDynSavePoll<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> {
    len: InnerLength,
    digest: D,
    state: State<Q, T, P, D>,
}

enum State<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> {
    Clean(P::Clean),
    Dirty(PairDynSavePoll<Q, T, P, D>),
    Done(Q),
}

#[doc(hidden)]
pub struct PairDynSavePoll<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> {
    left: PeakTreeDynSavePoll<Q, T, P, D>,
    right: PeakTreeDynSavePoll<Q, T, P, D>,
}

impl<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> PeakTreeDynSavePoll<Q, T, P, D> {
    fn encode_raw_node_blob(&self) -> raw::Node<T::DstBlob, Q, D> {
        match self {
            Self::Peak(leaf) => leaf.encode_raw_node_blob(),
            Self::Inner(inner) => inner.encode_raw_node_blob(),
        }
    }
}

impl<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> InnerDynSavePoll<Q, T, P, D> {
    fn encode_raw_node_blob(&self) -> raw::Node<T::DstBlob, Q, D> {
        match self.state {
            State::Done(ptr) => raw::Node::new(Some(self.digest), ptr),
            State::Clean(_) | State::Dirty(_) => panic!(),
        }
    }
}

impl<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> PairDynSavePoll<Q, T, P, D> {
    fn encode_raw_pair_blob(&self) -> raw::Pair<T::DstBlob, Q, D> {
        raw::Pair {
            left: self.left.encode_raw_node_blob(),
            right: self.right.encode_raw_node_blob(),
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRefPoll for PairDynSavePoll<Q, T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = PairDyn<T::DstBlob, Q, D>;

    fn blob_metadata(&self) -> InnerLength {
        let left_len: usize = self.left.blob_metadata().into();
        let right_len: usize = self.right.blob_metadata().into();
        // FIXME: should check errors or something here
        InnerLength::new(left_len | right_len).unwrap()
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        self.left.save_ref_poll(saver)?;
        self.right.save_ref_poll(saver)
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        dst.write_struct()
           .write_field(&self.encode_raw_pair_blob())
           .done()
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRefPoll for PeakTreeDynSavePoll<Q, T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = PeakTreeDyn<T::DstBlob, Q, D>;

    fn blob_metadata(&self) -> NonZeroLength {
        match self {
            Self::Peak(peak) => NonZeroLength::from_height(peak.blob_metadata()),
            Self::Inner(tip) => tip.blob_metadata().into(),
        }
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        match self {
            Self::Peak(peak) => peak.save_ref_poll(saver),
            Self::Inner(tip) => tip.save_ref_poll(saver),
        }
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        dst.write_struct()
           .write_field(&self.encode_raw_node_blob())
           .done()
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRefPoll for InnerDynSavePoll<Q, T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = InnerDyn<T::DstBlob, Q, D>;

    fn blob_metadata(&self) -> InnerLength {
        self.len
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = P::Clean, DstPtr = Q>
    {
        loop {
            self.state = match &mut self.state {
                State::Clean(p_clean) => {
                    match saver.save_ptr::<PairDyn<T, P, D>>(*p_clean, self.len)? {
                        Ok(q_ptr) => State::Done(q_ptr),
                        Err(target_poll) => State::Dirty(target_poll),
                    }
                },
                State::Dirty(target) => {
                    target.save_ref_poll(saver)?;

                    let q_ptr = saver.save_blob_with(target.blob_metadata(), |dst| {
                        target.encode_blob_dyn_bytes(dst)
                    })?;
                    State::Done(q_ptr)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        dst.write_struct()
           .write_field(&self.encode_raw_node_blob())
           .done()
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRef<Q> for PeakTreeDyn<T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = PeakTreeDyn<T::DstBlob, Q, D>;
    type SaveRefPoll = PeakTreeDynSavePoll<Q, T, P, D>;

    fn init_save_ref(&self) -> Self::SaveRefPoll {
        match self.kind() {
            Kind::Peak(leaf) => PeakTreeDynSavePoll::Peak(leaf.init_save_ref().into()),
            Kind::Inner(tip) => PeakTreeDynSavePoll::Inner(tip.init_save_ref().into()),
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRef<Q> for InnerDyn<T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = InnerDyn<T::DstBlob, Q, D>;
    type SaveRefPoll = InnerDynSavePoll<Q, T, P, D>;

    fn init_save_ref(&self) -> Self::SaveRefPoll {
        InnerDynSavePoll {
            len: self.len(),
            digest: self.pair_commit().digest(),
            state: match self.try_get_dirty_pair() {
                Ok(pair) => State::Dirty(pair.init_save_ref()),
                Err(p_clean) => State::Clean(p_clean),
            }
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SaveRef<Q> for PairDyn<T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = PairDyn<T::DstBlob, Q, D>;
    type SaveRefPoll = PairDynSavePoll<Q, T, P, D>;

    fn init_save_ref(&self) -> Self::SaveRefPoll {
        PairDynSavePoll {
            left: self.left().init_save_ref(),
            right: self.right().init_save_ref(),
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SavePoll for PeakTreeSavePoll<Q, T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = PeakTree<T::DstBlob, Q, D>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        self.0.save_ref_poll(saver)
    }

    fn encode_blob(&self) -> Self::DstBlob {
        let raw = self.0.encode_raw_node_blob();
        let len = self.0.blob_metadata();

        unsafe {
            PeakTree::from_raw_node(raw, len)
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> Save<Q> for PeakTree<T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = PeakTree<T::DstBlob, Q, D>;
    type SavePoll = PeakTreeSavePoll<Q, T, P, D>;

    fn init_save(&self) -> Self::SavePoll {
        PeakTreeSavePoll(
            self.deref().init_save_ref()
        )
    }
}





// -------- drop impls ------------
impl<T, P: Ptr, D: Digest> Drop for PeakTreeDyn<T, P, D> {
    fn drop(&mut self) {
        match self.kind_mut() {
            Kind::Peak(peak) => unsafe { ptr::drop_in_place(peak) },
            Kind::Inner(inner) => unsafe { ptr::drop_in_place(inner) },
        }
    }
}

impl<T, P: Ptr, D: Digest> Drop for PeakTree<T, P, D> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, P: Ptr, D: Digest> Drop for InnerDyn<T, P, D> {
    fn drop(&mut self) {
        let len = self.len();
        unsafe {
            self.raw.ptr.dealloc::<PairDyn<T, P, D>>(len);
        }
    }
}

impl<T, P: Ptr, D: Digest> Drop for Inner<T, P, D> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, P: Ptr, D: Digest> Drop for PairDyn<T, P, D> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, P: Ptr, D: Digest> Drop for Pair<T, P, D> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

// -------------- fmt::Debug impls ---------------

impl<T, P: Ptr, D: Digest> fmt::Debug for PeakTree<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, P: Ptr, D: Digest> fmt::Debug for PeakTreeDyn<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, P: Ptr, D: Digest> InnerDyn<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("digest", &self.raw.digest())
            .field("ptr", &self.try_get_dirty_pair()
                               .map_err(P::from_clean))
            .field("len", &self.len())
            .finish()
    }
}

impl<T, P: Ptr, D: Digest> fmt::Debug for Inner<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Inner", f)
    }
}

impl<T, P: Ptr, D: Digest> fmt::Debug for InnerDyn<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerDyn", f)
    }
}

impl<T, P: Ptr, D: Digest> PairDyn<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("left", &self.left())
            .field("right", &self.right())
            .field("len", &self.len())
            .finish()
    }
}

impl<T, P: Ptr, D: Digest> fmt::Debug for Pair<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Pair", f)
    }
}

impl<T, P: Ptr, D: Digest> fmt::Debug for PairDyn<T, P, D>
where T: fmt::Debug, P: fmt::Debug, D: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("PairDyn", f)
    }
}

/*
// ----- commit impls ----------
impl<T: Commit, P: Ptr, D: Digest> Commit for PeakTree<T, P, D> {
    type Commitment = PeakTree<T::Commitment>;

    fn to_commitment(&self) -> Self::Commitment {
        todo!()
    }
}

impl<T: Commit, P: Ptr, D: Digest> Commit for PeakTreeDyn<T, P, D> {
    type Commitment = PeakTree<T::Commitment>;

    fn to_commitment(&self) -> Self::Commitment {
        todo!()
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::{
        ptr::{
            Heap,
            key::{
                Map,
                offset::OffsetSaver,
            },
        },
    };

    #[test]
    fn from_peak() {
        let peak = PerfectTree::<u8, Heap>::new_leaf(42);
        let peaks = PeakTree::from(peak);
        assert_eq!(peaks.len(), 1);
    }

    #[test]
    fn save() {
        let peak = PerfectTree::<u8, Heap>::new_leaf(42);
        let peaks = PeakTree::from(peak);

        let saver = OffsetSaver::new(&[][..]);
        let (offset, buf) = saver.try_save(&peaks).unwrap();
        assert_eq!(offset, 1);
        assert_eq!(buf, vec![
            42, // u8 value

            // peak tree, which is a leaf
            42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // leaf digest
            0, 0, 0, 0, 0, 0, 0, 0, // leaf ptr
            1, 0, 0, 0, 0, 0, 0, 0, // len
        ]);
    }
}
