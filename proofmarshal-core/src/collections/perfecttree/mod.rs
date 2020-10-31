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
pub struct Pair<T, Z, P: Ptr = <Z as Zone>::Ptr> {
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
pub struct Tip<T, Z, P: Ptr = <Z as Zone>::Ptr> {
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
pub struct PerfectTree<T, Z, P: Ptr = <Z as Zone>::Ptr> {
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
}

/*
impl<T, Z: Zone> PerfectTreeDyn<T, Z>
where T: Load
{
    pub fn get<'a>(&'a self, idx: usize) -> Option<Ref<'a, T>>
        where Z: Get + AsZone<T::Zone>
    {
        match self.get_leaf(idx) {
            None => None,
            Some(Ref::Borrowed(leaf)) => Some(leaf.get()),
            Some(Ref::Owned(_leaf)) => todo!(),
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T>
        where Z: GetMut + AsZone<T::Zone>
    {
        match self.get_leaf_mut(idx) {
            None => None,
            Some(leaf) => Some(leaf.get_mut()),
        }
    }

    pub fn get_leaf<'a>(&'a self, idx: usize) -> Option<Ref<'a, Leaf<T, Z>>>
        where Z: Get
    {
        match self.kind() {
            Kind::Leaf(leaf) if idx == 0 => {
                Some(Ref::Borrowed(leaf))
            },
            Kind::Leaf(_) => None,
            Kind::Tip(tip) => tip.get_leaf(idx),
        }
    }

    pub fn get_leaf_mut(&mut self, idx: usize) -> Option<&mut Leaf<T, Z>>
        where Z: GetMut
    {
        match self.kind_mut() {
            Kind::Leaf(leaf) if idx == 0 => Some(leaf),
            Kind::Leaf(_) => None,
            Kind::Tip(tip) => tip.get_leaf_mut(idx),
        }
    }
}

use hoard::zone::heap::Heap;
pub fn test_get<'a>(tree: &'a PerfectTreeDyn<u8, Heap>, idx: usize) -> Option<Ref<'a, u8>> {
    tree.get(idx)
}

pub fn test_get_leaf<'a>(tree: &'a PerfectTreeDyn<u8, Heap>, idx: usize) -> Option<Ref<'a, Leaf<u8, Heap>>> {
    tree.get_leaf(idx)
}

pub fn test_get_leaf_mut<'a>(tree: &mut PerfectTreeDyn<u8, Heap>, idx: usize) -> Option<&mut Leaf<u8, Heap>> {
    tree.get_leaf_mut(idx)
}

pub fn test_drop(tree: PerfectTree<u8, Heap>) {
    let _ = tree;
}

impl<T, Z, P: Ptr> PerfectTreeDyn<T, Z, P> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    pub fn kind(&self) -> Kind<&Leaf<T, Z, P>, &InnerTipDyn<T, Z, P>> {
        unsafe { self.raw.kind(self.height()) }
    }

    pub fn kind_mut(&mut self) -> Kind<&mut Leaf<T, Z, P>, &mut InnerTipDyn<T, Z, P>> {
        unsafe { self.raw.kind_mut(self.height()) }
    }
}

pub enum InnerJoinError<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    HeightOverflow {
        left: PerfectTree<T, Z, P>,
        right: PerfectTree<T, Z, P>,
    },
    HeightMismatch {
        left: PerfectTree<T, Z, P>,
        right: PerfectTree<T, Z, P>,
    },
}

impl<T, Z: Zone> InnerTip<T, Z> {
    pub fn try_join_in(
        left: PerfectTree<T, Z>,
        right: PerfectTree<T, Z>,
        mut zone: impl BorrowMut<Z>,
    ) -> Result<Self, InnerJoinError<T, Z>>
        where Z: Alloc
    {
        let node = InnerNode::try_join(left, right)?;

        Ok(Self::new_unchecked(None, zone.borrow_mut().alloc(node)))
    }

    pub fn new_unchecked(digest: Option<Digest>, bag: Bag<InnerNodeDyn<T, Z>, Z>) -> Self {
        let (ptr, height, zone) = bag.into_raw_parts();

        let raw = InnerTipRaw::from_raw_parts(digest, zone, ptr);
        unsafe {
            Self::from_raw_parts(raw, height)
        }
    }
}

impl<T, Z, P: Ptr> InnerTip<T, Z, P> {
    pub unsafe fn from_raw_parts(raw: InnerTipRaw<T, Z, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw,
            height,
        }
    }

    pub fn into_raw_parts(self) -> (InnerTipRaw<T, Z, P>, NonZeroHeight) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.raw),
             ptr::read(&this.height))
        }
    }
}

impl<T, Z: Zone> InnerTipDyn<T, Z>
where T: Load
{
    pub fn get_leaf<'a>(&'a self, idx: usize) -> Option<Ref<'a, Leaf<T, Z>>>
        where Z: Get
    {
        if idx < self.len() {
            match self.get_node() {
                Ref::Borrowed(node) => node.get_leaf(idx),
                Ref::Owned(_node) => todo!(),
            }
        } else {
            None
        }
    }

    pub fn get_leaf_mut(&mut self, idx: usize) -> Option<&mut Leaf<T, Z>>
        where Z: GetMut
    {
        if idx < self.len() {
            self.get_node_mut()
                .get_leaf_mut(idx)
        } else {
            None
        }
    }

    pub fn get_node<'a>(&'a self) -> Ref<'a, InnerNodeDyn<T, Z>>
        where Z: Get
    {
        unsafe {
            self.raw.zone.get_unchecked(&self.raw.ptr, self.height())
        }.trust()
    }

    pub fn get_node_mut(&mut self) -> &mut InnerNodeDyn<T, Z>
        where Z: GetMut
    {
        let height = self.height();
        unsafe {
            self.raw.zone.get_unchecked_mut(&mut self.raw.ptr, height)
        }.trust()
    }
}

impl<T, Z, P: Ptr> InnerTipDyn<T, Z, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn len(&self) -> NonZeroLength {
        NonZeroLength::from_height(self.height())
    }

    pub fn digest(&self) -> &Digest {
        match self.try_digest() {
            Some(digest) => digest,
            None => self.calc_digest(),
        }
    }

    #[inline(never)]
    fn calc_digest(&self) -> &Digest {
        self.raw.digest.get_or_init(move || {
            todo!()
        })
    }

    pub fn try_digest(&self) -> Option<&Digest> {
        self.raw.digest.get()
    }

    pub fn try_get_dirty_node(&self) -> Result<&InnerNodeDyn<T, Z, P>, P::Clean> {
        unsafe {
            self.raw.ptr.try_get_dirty(self.height())
        }
    }
}

impl<T, Z: Zone> InnerNode<T, Z> {
    pub fn try_join(
        left: PerfectTree<T, Z>,
        right: PerfectTree<T, Z>
    ) -> Result<Self, InnerJoinError<T, Z>>
    {
        if left.height() != right.height() {
            Err(InnerJoinError::HeightMismatch { left, right })
        } else if let Some(height) = left.height().try_increment() {
            let (left, _) = left.into_raw_parts();
            let (right, _) = right.into_raw_parts();

            Ok(unsafe {
                Self::from_raw_parts(InnerNodeRaw::new(left, right), height)
            })
        } else {
            Err(InnerJoinError::HeightOverflow { left, right })
        }
    }
}

impl<T, Z, P: Ptr> InnerNode<T, Z, P> {
    pub unsafe fn from_raw_parts(raw: InnerNodeRaw<T, Z, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw,
            height,
        }
    }

    pub fn into_raw_parts(self) -> (InnerNodeRaw<T, Z, P>, NonZeroHeight) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.raw),
             ptr::read(&this.height))
        }
    }
}

impl<T, Z: Zone> InnerNodeDyn<T, Z>
where T: Load
{
    pub fn get_leaf<'a>(&'a self, idx: usize) -> Option<Ref<'a, Leaf<T, Z>>>
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

    pub fn get_leaf_mut(&mut self, idx: usize) -> Option<&mut Leaf<T, Z>>
        where Z: GetMut
    {
        let len = usize::from(self.len());
        if idx < len / 2 {
            self.left_mut().get_leaf_mut(idx)
        } else if idx < len {
            self.right_mut().get_leaf_mut(idx - (len / 2))
        } else {
            None
        }
    }
}

impl<T, Z, P: Ptr> InnerNodeDyn<T, Z, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn len(&self) -> NonZeroLength {
        NonZeroLength::from_height(self.height())
    }

    pub fn left(&self) -> &PerfectTreeDyn<T, Z, P> {
        unsafe { self.raw.left(self.height()) }
    }

    pub fn left_mut(&mut self) -> &mut PerfectTreeDyn<T, Z, P> {
        unsafe { self.raw.left_mut(self.height()) }
    }

    pub fn right(&self) -> &PerfectTreeDyn<T, Z, P> {
        unsafe { self.raw.right(self.height()) }
    }

    pub fn right_mut(&mut self) -> &mut PerfectTreeDyn<T, Z, P> {
        unsafe { self.raw.right_mut(self.height()) }
    }
}





// ---------- Debug impls ----------
impl<T, Z, P: Ptr> fmt::Debug for PerfectTreeDyn<T, Z, P>
where T: fmt::Debug,
      Z: fmt::Debug,
      P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            Kind::Leaf(leaf) => f.debug_tuple("Leaf")
                                 .field(leaf)
                                 .finish(),
            Kind::Tip(tip) => f.debug_tuple("Tip")
                               .field(&tip)
                               .finish(),
        }
    }
}

impl<T, Z, P: Ptr> fmt::Debug for PerfectTree<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}

impl<T, Z, P: Ptr> InnerTipDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("digest", &self.raw.digest)
            .field("zone", &self.raw.zone)
            .field("ptr", &self.try_get_dirty_node()
                               .map_err(P::from_clean))
            .field("height", &self.height())
            .finish()
    }
}

impl<T, Z, P: Ptr> fmt::Debug for InnerTipDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerTipDyn", f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for InnerTip<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerTip", f)
    }
}

impl<T, Z, P: Ptr> InnerNodeDyn<T, Z, P>
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

impl<T, Z, P: Ptr> fmt::Debug for InnerNodeDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerNodeDyn", f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for InnerNode<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerNode", f)
    }
}


// -------- hoard blob impls --------------

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeInnerTipDynBytesError<Zone: fmt::Debug, Ptr: fmt::Debug> {
    Zone(Zone),
    Ptr(Ptr),
}

unsafe impl<T, Z, P: PtrBlob> BlobDyn for InnerTipDyn<T, Z, P>
where T: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeInnerTipDynBytesError<Z::DecodeBytesError, P::DecodeBytesError>;
    fn try_size(_: Self::Metadata) -> Result<usize, !> {
        todo!()
    }

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeInnerTipBytesError<Zone: fmt::Debug, Ptr: fmt::Debug> {
    Height(<NonZeroHeight as Blob>::DecodeBytesError),
    Inner(DecodeInnerTipDynBytesError<Zone, Ptr>),
}

impl<T, Z, P: PtrBlob> Blob for InnerTip<T, Z, P>
where T: Blob,
      Z: Blob,
{
    const SIZE: usize = <NonZeroHeight as Blob>::SIZE + <Digest as Blob>::SIZE + <Z as Blob>::SIZE + <P as Blob>::SIZE;
    type DecodeBytesError = DecodeInnerTipBytesError<Z::DecodeBytesError, P::DecodeBytesError>;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeInnerNodeDynBytesError<Left: fmt::Debug, Right: fmt::Debug> {
    Left(Left),
    Right(Right),
}

unsafe impl<T, Z, P: PtrBlob> BlobDyn for InnerNodeDyn<T, Z, P>
where T: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeInnerNodeDynBytesError<!, !>;
    fn try_size(_: Self::Metadata) -> Result<usize, !> {
        todo!()
    }

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum PerfectTreeDynBytesError<Leaf: fmt::Debug, Tip: fmt::Debug> {
    Leaf(Leaf),
    Tip(Tip),
}

unsafe impl<T, Z, P: PtrBlob> BlobDyn for PerfectTreeDyn<T, Z, P>
where T: Blob,
      Z: Blob,
{
    type DecodeBytesError = PerfectTreeDynBytesError<
        <Leaf<T, Z, P> as Blob>::DecodeBytesError,
        <InnerTipDyn<T, Z, P> as BlobDyn>::DecodeBytesError,
    >;

    fn try_size(_: Self::Metadata) -> Result<usize, !> {
        todo!()
    }

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum PerfectTreeBytesError<Leaf: fmt::Debug, Tip: fmt::Debug> {
    Height(<Height as Blob>::DecodeBytesError),
    Inner(PerfectTreeDynBytesError<Leaf, Tip>),
}

impl<T, Z, P: PtrBlob> Blob for PerfectTree<T, Z, P>
where T: Blob,
      Z: Blob,
{
    const SIZE: usize = <Height as Blob>::SIZE + <Leaf<T, Z, P> as Blob>::SIZE;
    type DecodeBytesError = PerfectTreeBytesError<
        <Leaf<T, Z, P> as Blob>::DecodeBytesError,
        <InnerTipDyn<T, Z, P> as BlobDyn>::DecodeBytesError,
    >;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> { todo!() }
}

// -------- hoard load impls --------------
impl<T, Z, P: Ptr> Load for PerfectTree<T, Z, P>
where T: Load,
      Z: Zone,
{
    type Blob = PerfectTree<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        match blob.into_kind() {
            Kind::Leaf(leaf) => Self::from_leaf(Load::load(leaf, zone)),
            Kind::Tip(_tip) => todo!(), //Self::from_tip(tip.load(zone)),
        }
    }
}

impl<T, Z, P: Ptr> Load for InnerTip<T, Z, P>
where T: Load,
      Z: Zone,
{
    type Blob = InnerTip<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(_blob: Self::Blob, _zone: &Self::Zone) -> Self {
        todo!()
    }
}

impl<T, Z, P: Ptr> LoadRef for InnerNodeDyn<T, Z, P>
where T: Load,
      Z: Zone,
{
    type BlobDyn = InnerTipDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(_: Bytes<'a, Self::BlobDyn>, _: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>,
                 <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        todo!()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test() {
        let leaf1 = PerfectTree::from_leaf(Leaf::new_in(1u8, Heap));
        assert_eq!(leaf1.height(), 0);

        let leaf2 = PerfectTree::from_leaf(Leaf::new_in(2u8, Heap));
        assert_eq!(leaf1.height(), 0);

        let tip = InnerTip::try_join_in(leaf1, leaf2, Heap).ok().unwrap();
        assert_eq!(tip.height(), 1);

        let tip = PerfectTree::from_tip(tip);
        assert_eq!(tip.get(0),
                   Some(Ref::Owned(1)));
        assert_eq!(tip.get(1),
                   Some(Ref::Owned(2)));
        assert_eq!(tip.get(2), None);
    }
}

/*
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
*/
