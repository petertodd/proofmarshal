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
use hoard::save::{SaveDirty, SaveDirtyPoll, SaveDirtyRef, SaveDirtyRefPoll, BlobSaver};
use hoard::zone::{Alloc, AsZone, Zone, Get, GetMut, Ptr, PtrConst, PtrBlob, FromPtr};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, Own, Ref};
use hoard::bag::Bag;

use crate::collections::merklesum::MerkleSum;
use crate::commit::{Commit, WriteVerbatim, Digest};
use crate::unreachable_unchecked;

use super::height::*;
use super::length::*;
use super::raw;
use super::leaf::Leaf;

#[repr(C)]
pub struct Pair<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, Z, P>>,
    height: NonZeroHeight,
}

#[repr(C)]
pub struct PairDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, Z, P>>,
    height: NonZeroHeightDyn,
}

#[repr(C)]
pub struct Tip<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    height: NonZeroHeight,
}

#[repr(C)]
pub struct TipDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    height: NonZeroHeightDyn,
}

#[repr(C)]
pub struct PerfectTree<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    height: Height,
}

#[repr(C)]
pub struct PerfectTreeDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    height: HeightDyn,
}

#[derive(Debug)]
pub enum Kind<Leaf, Tip> {
    Leaf(Leaf),
    Tip(Tip),
}

impl<T, Z: Zone> PerfectTree<T, Z> {
    pub fn try_join(left: PerfectTree<T, Z>, right: PerfectTree<T, Z>) -> Result<Self, JoinError<T, Z>>
        where Z: Alloc
    {
        let tip = Tip::try_join(left, right)?;
        Ok(Self::from(tip))
    }

    pub fn new_leaf_in(value: T, zone: Z) -> Self
        where Z: Alloc
    {
        Self::from(Leaf::new_in(value, zone))
    }
}

impl<T, Z, P: Ptr> From<Leaf<T, Z, P>> for PerfectTree<T, Z, P> {
    fn from(leaf: Leaf<T, Z, P>) -> Self {
        let raw = leaf.into_raw();
        unsafe {
            Self::from_raw_node(raw, Height::ZERO)
        }
    }
}

impl<T, Z, P: Ptr> From<Tip<T, Z, P>> for PerfectTree<T, Z, P> {
    fn from(tip: Tip<T, Z, P>) -> Self {
        let height = tip.height().into();
        let raw = tip.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, height)
        }
    }
}

impl<T, Z, P: Ptr> PerfectTree<T, Z, P> {
    pub fn into_kind(self) -> Kind<Leaf<T, Z, P>, Tip<T, Z, P>> {
        let height = self.height();
        let node = self.into_raw_node();

        if let Ok(height) = NonZeroHeight::try_from(height) {
            let tip = unsafe { Tip::from_raw_node(node, height) };
            Kind::Tip(tip)
        } else {
            let leaf = unsafe { Leaf::from_raw(node) };
            Kind::Leaf(leaf)
        }
    }
}

impl<T, Z: Zone> PerfectTreeDyn<T, Z>
where T: Load
{
    pub fn get(&self, idx: usize) -> Option<Ref<T>>
        where Z: Get + AsZone<T::Zone>
    {
        self.get_leaf(idx).map(|leaf| {
            match leaf {
                Ref::Borrowed(leaf) => leaf.get(),
                Ref::Owned(leaf) => Ref::Owned(leaf.take()),
            }
        })
    }

    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, Z>>>
        where Z: Get
    {
        match self.kind() {
            Kind::Leaf(leaf) if idx == 0 => Some(Ref::Borrowed(leaf)),
            Kind::Leaf(_) => None,
            Kind::Tip(tip) => tip.get_leaf(idx),
        }
    }
}

impl<T, Z, P: Ptr> PerfectTreeDyn<T, Z, P> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    pub fn kind(&self) -> Kind<&Leaf<T, Z, P>, &TipDyn<T, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let tip = unsafe { TipDyn::from_raw_node_ref(&self.raw, height) };
            Kind::Tip(tip)
        } else {
            let leaf = unsafe { Leaf::from_raw_node_ref(&self.raw) };
            Kind::Leaf(leaf)
        }
    }

    pub fn kind_mut(&mut self) -> Kind<&mut Leaf<T, Z, P>, &mut TipDyn<T, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let tip = unsafe { TipDyn::from_raw_node_mut(&mut self.raw, height) };
            Kind::Tip(tip)
        } else {
            let leaf = unsafe { Leaf::from_raw_node_mut(&mut self.raw) };
            Kind::Leaf(leaf)
        }
    }

    pub fn node_digest(&self) -> Digest
        where T: Commit
    {
        match self.kind() {
            Kind::Leaf(leaf) => leaf.digest().cast(),
            Kind::Tip(tip) => tip.pair_digest().cast(),
        }
    }

    pub fn try_node_digest(&self) -> Option<Digest>
    {
        match self.kind() {
            Kind::Leaf(leaf) => leaf.try_digest(),
            Kind::Tip(tip) => tip.try_pair_digest(),
        }
    }

    pub fn zone(&self) -> Z
        where Z: Copy
    {
        self.raw.zone
    }
}

impl<T, Z: Zone> Tip<T, Z> {
    pub fn try_join(left: PerfectTree<T, Z>, right: PerfectTree<T, Z>) -> Result<Self, JoinError<T, Z>>
        where Z: Alloc
    {
        let pair = Pair::try_join(left, right)?;
        Ok(Self::new(pair))
    }

    pub fn new(pair: Pair<T, Z>) -> Self
        where Z: Alloc
    {
        let zone = pair.zone();
        Self::new_unchecked(None, zone.alloc(pair))
    }

    pub fn new_unchecked(digest: Option<Digest>, pair: Bag<PairDyn<T, Z>, Z>) -> Self {
        let (ptr, height, zone) = pair.into_raw_parts();
        let raw = raw::Node::new(digest, zone, ptr);

        unsafe {
            Self::from_raw_node(raw, height)
        }
    }
}

impl<T, Z: Zone> TipDyn<T, Z>
where T: Load
{
    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, Z>>>
        where Z: Get
    {
        match self.get_pair() {
            Ref::Borrowed(pair) => pair.get_leaf(idx),
            Ref::Owned(_owned) => todo!(),
        }
    }

    pub fn get_pair(&self) -> Ref<PairDyn<T, Z>>
        where Z: Get
    {
        unsafe {
            self.raw.get_unchecked(self.height())
                    .trust()
        }
    }

    pub fn get_pair_mut(&mut self) -> &mut PairDyn<T, Z>
        where Z: GetMut
    {
        let height = self.height();
        unsafe {
            self.raw.get_unchecked_mut(height)
                    .trust()
        }
    }
}

impl<T, Z, P: Ptr> TipDyn<T, Z, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn pair_digest(&self) -> Digest<Pair<T::Committed>>
        where T: Commit
    {
        self.try_pair_digest()
            .map(|digest| digest.cast())
            .unwrap_or_else(|| self.calc_pair_digest())
    }

    fn calc_pair_digest(&self) -> Digest<Pair<T::Committed>>
        where T: Commit
    {
        let pair = self.try_get_dirty_pair()
                       .ok().expect("digest missing yet tip ptr clean");
        let digest = pair.commit();
        self.raw.set_digest(digest.cast());
        digest
    }

    pub fn try_pair_digest(&self) -> Option<Digest>
    {
        self.raw.digest()
    }

    pub fn zone(&self) -> Z
        where Z: Copy
    {
        self.raw.zone
    }
}

#[derive(Debug)]
pub enum JoinError<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    Overflow {
        left: PerfectTree<T, Z, P>,
        right: PerfectTree<T, Z, P>,
    },
    Mismatch {
        left: PerfectTree<T, Z, P>,
        right: PerfectTree<T, Z, P>,
    }
}

impl<T, Z: Zone> Pair<T, Z> {
    pub fn try_join(left: PerfectTree<T, Z>, right: PerfectTree<T, Z>) -> Result<Self, JoinError<T, Z>> {
        if left.height() != right.height() {
            Err(JoinError::Mismatch { left, right })
        } else if let Some(height) = left.height().try_increment() {
            let pair = raw::Pair {
                left: left.into_raw_node(),
                right: right.into_raw_node(),
            };

            Ok(unsafe { Self::from_raw_pair(pair, height) })
        } else {
            Err(JoinError::Overflow { left, right })
        }
    }
}

impl<T, Z: Zone> PairDyn<T, Z>
where T: Load
{
    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, Z>>>
        where Z: Get
    {
        let len = usize::from(self.len());
        if idx < len / 2 {
            self.left().get_leaf(idx)
        } else if idx < len {
            self.right().get_leaf(idx - (len / 2))
        } else {
            None
        }
    }
}

impl<T, Z, P: Ptr> PairDyn<T, Z, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn len(&self) -> NonZeroLength {
        NonZeroLength::from_height(self.height())
    }

    pub fn zone(&self) -> Z
        where Z: Copy
    {
        self.raw.left.zone
    }

    pub fn left(&self) -> &PerfectTreeDyn<T, Z, P> {
        unsafe {
            PerfectTreeDyn::from_raw_node_ref(&self.raw.left, self.height().decrement())
        }
    }

    pub fn left_mut(&mut self) -> &mut PerfectTreeDyn<T, Z, P> {
        let height = self.height().decrement();
        unsafe {
            PerfectTreeDyn::from_raw_node_mut(&mut self.raw.left, height)
        }
    }

    pub fn right(&self) -> &PerfectTreeDyn<T, Z, P> {
        unsafe {
            PerfectTreeDyn::from_raw_node_ref(&self.raw.right, self.height().decrement())
        }
    }

    pub fn right_mut(&mut self) -> &mut PerfectTreeDyn<T, Z, P> {
        let height = self.height().decrement();
        unsafe {
            PerfectTreeDyn::from_raw_node_mut(&mut self.raw.right, height)
        }
    }
}

// --------- conversions from raw -------------

impl<T, Z, P: Ptr> Pair<T, Z, P> {
    pub unsafe fn from_raw_pair(raw: raw::Pair<T, Z, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_pair(self) -> raw::Pair<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> PairDyn<T, Z, P> {
    pub unsafe fn from_raw_pair_ref(raw: &raw::Pair<T, Z, P>, height: NonZeroHeight) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_pair_mut(raw: &mut raw::Pair<T, Z, P>, height: NonZeroHeight) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }
}

impl<T, Z, P: Ptr> Tip<T, Z, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, Z, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> TipDyn<T, Z, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, Z, P>, height: NonZeroHeight) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, Z, P>, height: NonZeroHeight) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }

    pub fn try_get_dirty_pair(&self) -> Result<&PairDyn<T, Z, P>, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(self.height())
        }
    }
}

impl<T, Z, P: Ptr> PerfectTree<T, Z, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, Z, P>, height: Height) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> PerfectTreeDyn<T, Z, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, Z, P>, height: Height) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, Z, P>, height: Height) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }
}

// ------- pointee impls ----------

macro_rules! impl_pointee {
    ($t:ident, $meta_ty:ty) => {
        impl<T, Z, P: Ptr> Pointee for $t<T, Z, P> {
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

            fn make_fat_ptr(thin: *const (), height: Self::Metadata) -> *const Self {
                let height = height.get();
                let height: u8 = height.into();
                let ptr = ptr::slice_from_raw_parts(thin, height.into());
                unsafe { mem::transmute(ptr) }
            }

            fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
                let height = height.get();
                let height: u8 = height.into();
                let ptr = ptr::slice_from_raw_parts_mut(thin, height.into());
                unsafe { mem::transmute(ptr) }
            }
        }
    }
}

impl_pointee!(PerfectTreeDyn, Height);
impl_pointee!(TipDyn, NonZeroHeight);
impl_pointee!(PairDyn, NonZeroHeight);

// --------- deref impls ----------

macro_rules! impl_deref {
    ($t:ident => $u:ident) => {
        impl<T, Z, P: Ptr> Borrow<$u<T, Z, P>> for $t<T, Z, P> {
            fn borrow(&self) -> &$u<T, Z, P> {
                unsafe {
                    &*$u::make_fat_ptr(self as *const _ as *const (), self.height)
                }
            }
        }

        impl<T, Z, P: Ptr> BorrowMut<$u<T, Z, P>> for $t<T, Z, P> {
            fn borrow_mut(&mut self) -> &mut $u<T, Z, P> {
                unsafe {
                    &mut *$u::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
                }
            }
        }

        unsafe impl<T, Z, P: Ptr> Take<$u<T, Z, P>> for $t<T, Z, P> {
            fn take_unsized<F, R>(self, f: F) -> R
                where F: FnOnce(Own<$u<T, Z, P>>) -> R
            {
                let mut this = ManuallyDrop::new(self);
                let this_dyn: &mut $u<T, Z, P> = this.deref_mut().borrow_mut();

                unsafe {
                    f(Own::new_unchecked(this_dyn))
                }
            }
        }

        impl<T, Z, P: Ptr> IntoOwned for $u<T, Z, P> {
            type Owned = $t<T, Z, P>;

            fn into_owned(self: Own<'_, Self>) -> Self::Owned {
                let this = Own::leak(self);
                unsafe {
                    $t {
                        marker: PhantomData,
                        height: this.height(),
                        raw: ptr::read(&this.raw),
                    }
                }
            }
        }

        impl<T, Z, P: Ptr> Deref for $t<T, Z, P> {
            type Target = $u<T, Z, P>;

            fn deref(&self) -> &Self::Target {
                self.borrow()
            }
        }

        impl<T, Z, P: Ptr> DerefMut for $t<T, Z, P> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.borrow_mut()
            }
        }
    }
}

impl_deref!(PerfectTree => PerfectTreeDyn);
impl_deref!(Tip => TipDyn);
impl_deref!(Pair => PairDyn);

// ------- hoard impls ----------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePerfectTreeBytesError<Raw: error::Error, Height: error::Error> {
    Raw(Raw),
    Height(Height),
}

impl<T, Z, P: Ptr> Blob for PerfectTree<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, Z, P> as Blob>::SIZE + <Height as Blob>::SIZE;
    type DecodeBytesError = DecodePerfectTreeBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError, <Height as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.height)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let height = fields.trust_field().map_err(Self::DecodeBytesError::Height)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_node(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> Load for PerfectTree<T, Z, P>
where T: Load
{
    type Blob = PerfectTree<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let height = blob.height;
        let raw = blob.into_raw_node();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_node(raw, height) }
    }
}


#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodePerfectTreeDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, Z, P: Ptr> BlobDyn for PerfectTreeDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodePerfectTreeDynBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<PerfectTree<T, Z, P> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let height = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodePerfectTreeDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { PerfectTree::from_raw_node(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> LoadRef for PerfectTreeDyn<T, Z, P>
where T: Load
{
    type BlobDyn = PerfectTreeDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = PerfectTree::<T, Z, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeTipBytesError<Raw: error::Error, Height: error::Error> {
    Raw(Raw),
    Height(Height),
}

impl<T, Z, P: Ptr> Blob for Tip<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, Z, P> as Blob>::SIZE + <NonZeroHeight as Blob>::SIZE;
    type DecodeBytesError = DecodeTipBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError, <NonZeroHeight as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.height)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let height = fields.trust_field().map_err(Self::DecodeBytesError::Height)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_node(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> Load for Tip<T, Z, P>
where T: Load
{
    type Blob = Tip<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let height = blob.height;
        let raw = blob.into_raw_node();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_node(raw, height) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodeTipDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, Z, P: Ptr> BlobDyn for TipDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodeTipDynBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<Tip<T, Z, P> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let height = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodeTipDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { Tip::from_raw_node(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> LoadRef for TipDyn<T, Z, P>
where T: Load
{
    type BlobDyn = TipDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = Tip::<T, Z, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePairBytesError<Raw: error::Error, Height: error::Error> {
    Raw(Raw),
    Height(Height),
}

impl<T, Z, P: Ptr> Blob for Pair<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Pair<T, Z, P> as Blob>::SIZE + <NonZeroHeight as Blob>::SIZE;
    type DecodeBytesError = DecodePairBytesError<<raw::Pair<T, Z, P> as Blob>::DecodeBytesError, <NonZeroHeight as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .write_field(&self.height)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(Self::DecodeBytesError::Raw)?;
        let height = fields.trust_field().map_err(Self::DecodeBytesError::Height)?;
        fields.assert_done();
        Ok(unsafe { Self::from_raw_pair(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> Load for Pair<T, Z, P>
where T: Load
{
    type Blob = Pair<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let height = blob.height;
        let raw = blob.into_raw_pair();
        let raw = Load::load(raw, zone);
        unsafe { Self::from_raw_pair(raw, height) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodePairDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, Z, P: Ptr> BlobDyn for PairDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodePairDynBytesError<<raw::Pair<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<Pair<T, Z, P> as Blob>::SIZE)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&*self.raw)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        let height = src.metadata();
        let mut fields = src.struct_fields();
        let raw = fields.trust_field().map_err(DecodePairDynBytesError)?;
        fields.assert_done();
        Ok(unsafe { Pair::from_raw_pair(raw, height) }.into())
    }
}

impl<T, Z: Zone, P: Ptr> LoadRef for PairDyn<T, Z, P>
where T: Load
{
    type BlobDyn = PairDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;

        let owned = Pair::<T, Z, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

// -------- drop impls ------------
impl<T, Z, P: Ptr> Drop for PerfectTreeDyn<T, Z, P> {
    fn drop(&mut self) {
        match self.kind_mut() {
            Kind::Leaf(leaf) => unsafe { ptr::drop_in_place(leaf) },
            Kind::Tip(tip) => unsafe { ptr::drop_in_place(tip) },
        }
    }
}

impl<T, Z, P: Ptr> Drop for PerfectTree<T, Z, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, Z, P: Ptr> Drop for TipDyn<T, Z, P> {
    fn drop(&mut self) {
        let height = self.height();
        unsafe {
            self.raw.ptr.dealloc::<PairDyn<T, Z, P>>(height);
        }
    }
}

impl<T, Z, P: Ptr> Drop for Tip<T, Z, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, Z, P: Ptr> Drop for PairDyn<T, Z, P> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, Z, P: Ptr> Drop for Pair<T, Z, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}


// -------------- fmt::Debug impls ---------------

impl<T, Z, P: Ptr> fmt::Debug for PerfectTree<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for PerfectTreeDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, Z, P: Ptr> TipDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("digest", &self.raw.digest())
            .field("zone", &self.raw.zone)
            .field("ptr", &self.try_get_dirty_pair()
                               .map_err(P::from_clean))
            .field("height", &self.height())
            .finish()
    }
}

impl<T, Z, P: Ptr> fmt::Debug for Tip<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Tip", f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for TipDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("TipDyn", f)
    }
}

impl<T, Z, P: Ptr> PairDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &self.height())
            .finish()
    }
}

impl<T, Z, P: Ptr> fmt::Debug for Pair<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Pair", f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for PairDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("PairDyn", f)
    }
}

// --------- commit impls -----------

impl<T, Z, P: Ptr> Commit for PerfectTreeDyn<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = PerfectTree<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        match self.kind() {
            Kind::Leaf(leaf) => {
                dst.write(&leaf.commit().as_bytes());
                dst.write(&0u8);
            },
            Kind::Tip(tip) => {
                tip.encode_verbatim(dst)
            },
        }
    }
}

impl<T, Z, P: Ptr> Commit for PerfectTree<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = PerfectTree<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}

impl<T, Z, P: Ptr> Commit for TipDyn<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Tip<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.pair_digest().as_bytes());
        dst.write(&self.height());
    }
}

impl<T, Z, P: Ptr> Commit for Tip<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Tip<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}

impl<T, Z, P: Ptr> Commit for PairDyn<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = (Digest::<!>::LEN * 2) + 1;
    type Committed = Pair<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.left().node_digest().as_bytes());
        dst.write(&self.right().node_digest().as_bytes());
        dst.write(&self.height());
    }
}

impl<T, Z, P: Ptr> Commit for Pair<T, Z, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Pair<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test_get() {
        let leaf0 = PerfectTree::new_leaf_in(0u8, Heap);
        let leaf1 = PerfectTree::new_leaf_in(1u8, Heap);
        let tree0 = PerfectTree::try_join(leaf0, leaf1).unwrap();
        assert_eq!(tree0.get(0).unwrap(), &0);
        assert_eq!(tree0.get(1).unwrap(), &1);
        assert_eq!(tree0.get(2), None);
        assert_eq!(tree0.get(usize::MAX), None);
    }

    #[test]
    fn test_commit() {
        let n = 0u8;
        let d_n = n.commit();

        let leaf0 = PerfectTree::new_leaf_in(0u8, Heap);
        let leaf1 = PerfectTree::new_leaf_in(1u8, Heap);
        let tree0 = PerfectTree::try_join(leaf0, leaf1).unwrap();

        dbg!(tree0.commit());
        dbg!(tree0.to_verbatim());
    }
}
