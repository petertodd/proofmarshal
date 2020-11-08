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

use crate::collections::merklesum::MerkleSum;
use crate::commit::{Commit, WriteVerbatim, Digest};
use crate::unreachable_unchecked;

use super::height::*;
use super::length::*;
use super::raw;
use super::leaf::Leaf;

#[repr(C)]
pub struct Pair<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, P>>,
    height: NonZeroHeight,
}

#[repr(C)]
pub struct PairDyn<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, P>>,
    height: NonZeroHeightDyn,
}

#[repr(C)]
pub struct Tip<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P>>,
    height: NonZeroHeight,
}

#[repr(C)]
pub struct TipDyn<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P>>,
    height: NonZeroHeightDyn,
}

#[repr(C)]
pub struct PerfectTree<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P>>,
    height: Height,
}

#[repr(C)]
pub struct PerfectTreeDyn<T, P: Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, P>>,
    height: HeightDyn,
}

#[derive(Debug)]
pub enum Kind<Leaf, Tip> {
    Leaf(Leaf),
    Tip(Tip),
}

impl<T, P: Ptr> PerfectTree<T, P> {
    pub fn try_join(left: PerfectTree<T, P>, right: PerfectTree<T, P>) -> Result<Self, (PerfectTree<T, P>, PerfectTree<T, P>)>
        where P: Default
    {
        let tip = Tip::try_join(left, right)?;
        Ok(Self::from(tip))
    }

    pub fn new_leaf(value: T) -> Self
        where P: Default
    {
        Self::from(Leaf::new(value))
    }
}

impl<T, P: Ptr> From<Leaf<T, P>> for PerfectTree<T, P> {
    fn from(leaf: Leaf<T, P>) -> Self {
        let raw = leaf.into_raw();
        unsafe {
            Self::from_raw_node(raw, Height::ZERO)
        }
    }
}

impl<T, P: Ptr> From<Tip<T, P>> for PerfectTree<T, P> {
    fn from(tip: Tip<T, P>) -> Self {
        let height = tip.height().into();
        let raw = tip.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, height)
        }
    }
}

impl<T, P: Ptr> PerfectTree<T, P> {
    pub fn into_kind(self) -> Kind<Leaf<T, P>, Tip<T, P>> {
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

impl<T, P: Ptr> PerfectTreeDyn<T, P>
where T: Load,
      P::Zone: AsZone<T::Zone>,
{
    pub fn get(&self, idx: usize) -> Option<Ref<T>>
        where P: Get
    {
        self.get_leaf(idx).map(|leaf| {
            match leaf {
                Ref::Borrowed(leaf) => leaf.get(),
                Ref::Owned(leaf) => Ref::Owned(leaf.take()),
            }
        })
    }

    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, P>>>
        where P: Get
    {
        match self.kind() {
            Kind::Leaf(leaf) if idx == 0 => Some(Ref::Borrowed(leaf)),
            Kind::Leaf(_) => None,
            Kind::Tip(tip) => tip.get_leaf(idx),
        }
    }
}

impl<T, P: Ptr> PerfectTreeDyn<T, P> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    pub fn len(&self) -> NonZeroLength {
        NonZeroLength::from_height(self.height())
    }

    pub fn kind(&self) -> Kind<&Leaf<T, P>, &TipDyn<T, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let tip = unsafe { TipDyn::from_raw_node_ref(&self.raw, height) };
            Kind::Tip(tip)
        } else {
            let leaf = unsafe { Leaf::from_raw_node_ref(&self.raw) };
            Kind::Leaf(leaf)
        }
    }

    pub fn kind_mut(&mut self) -> Kind<&mut Leaf<T, P>, &mut TipDyn<T, P>> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let tip = unsafe { TipDyn::from_raw_node_mut(&mut self.raw, height) };
            Kind::Tip(tip)
        } else {
            let leaf = unsafe { Leaf::from_raw_node_mut(&mut self.raw) };
            Kind::Leaf(leaf)
        }
    }

    /*
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
    */
}

impl<T, P: Ptr> Tip<T, P> {
    pub fn try_join(left: PerfectTree<T, P>, right: PerfectTree<T, P>) -> Result<Self, (PerfectTree<T, P>, PerfectTree<T, P>)>
        where P: Default
    {
        let pair = Pair::try_join(left, right)?;
        Ok(Self::new(pair))
    }

    pub fn new(pair: Pair<T, P>) -> Self
        where P: Default
    {
        Self::new_unchecked(None, P::alloc(pair))
    }

    pub fn new_unchecked(digest: Option<Digest>, pair: Bag<PairDyn<T, P>, P>) -> Self {
        let (ptr, height) = pair.into_raw_parts();
        let raw = raw::Node::new(digest, ptr);

        unsafe {
            Self::from_raw_node(raw, height)
        }
    }
}

impl<T, P: Ptr> TipDyn<T, P>
where T: Load,
      P::Zone: AsZone<T::Zone>,
{
    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, P>>>
        where P: Get
    {
        match self.get_pair() {
            Ref::Borrowed(pair) => pair.get_leaf(idx),
            Ref::Owned(_owned) => todo!(),
        }
    }

    pub fn get_pair(&self) -> Ref<PairDyn<T, P>>
        where P: Get
    {
        unsafe {
            self.raw.get::<PairDyn<T, P>>(self.height())
                    .trust()
        }
    }

    pub fn get_pair_mut(&mut self) -> &mut PairDyn<T, P>
        where P: GetMut
    {
        let height = self.height();
        unsafe {
            self.raw.get_mut::<PairDyn<T, P>>(height)
                    .trust()
        }
    }
}

impl<T, P: Ptr> TipDyn<T, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    /*
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
    */
}

impl<T, P: Ptr> Pair<T, P> {
    pub fn try_join(left: PerfectTree<T, P>, right: PerfectTree<T, P>) -> Result<Self, (PerfectTree<T, P>, PerfectTree<T, P>)> {
        if left.height() != right.height() {
            panic!("height mismatch")
        } else if let Some(height) = left.height().try_increment() {
            let pair = raw::Pair {
                left: left.into_raw_node(),
                right: right.into_raw_node(),
            };

            Ok(unsafe { Self::from_raw_pair(pair, height) })
        } else {
            Err((left, right))
        }
    }
}

impl<T, P: Ptr> PairDyn<T, P>
where T: Load,
      P::Zone: AsZone<T::Zone>,
{
    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, P>>>
        where P: Get
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

impl<T, P: Ptr> PairDyn<T, P> {
    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }

    pub fn len(&self) -> NonZeroLength {
        NonZeroLength::from_height(self.height())
    }

    pub fn left(&self) -> &PerfectTreeDyn<T, P> {
        unsafe {
            PerfectTreeDyn::from_raw_node_ref(&self.raw.left, self.height().decrement())
        }
    }

    pub fn left_mut(&mut self) -> &mut PerfectTreeDyn<T, P> {
        let height = self.height().decrement();
        unsafe {
            PerfectTreeDyn::from_raw_node_mut(&mut self.raw.left, height)
        }
    }

    pub fn right(&self) -> &PerfectTreeDyn<T, P> {
        unsafe {
            PerfectTreeDyn::from_raw_node_ref(&self.raw.right, self.height().decrement())
        }
    }

    pub fn right_mut(&mut self) -> &mut PerfectTreeDyn<T, P> {
        let height = self.height().decrement();
        unsafe {
            PerfectTreeDyn::from_raw_node_mut(&mut self.raw.right, height)
        }
    }
}

// --------- conversions from raw -------------

impl<T, P: Ptr> Pair<T, P> {
    pub unsafe fn from_raw_pair(raw: raw::Pair<T, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_pair(self) -> raw::Pair<T, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr> PairDyn<T, P> {
    pub unsafe fn from_raw_pair_ref(raw: &raw::Pair<T, P>, height: NonZeroHeight) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_pair_mut(raw: &mut raw::Pair<T, P>, height: NonZeroHeight) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }
}

impl<T, P: Ptr> Tip<T, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, P>, height: NonZeroHeight) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr> TipDyn<T, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, P>, height: NonZeroHeight) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, P>, height: NonZeroHeight) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }

    pub fn try_get_dirty_pair(&self) -> Result<&PairDyn<T, P>, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(self.height())
                    .map(MaybeValid::trust)
        }
    }
}

impl<T, P: Ptr> PerfectTree<T, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, P>, height: Height) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            height,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, P: Ptr> PerfectTreeDyn<T, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, P>, height: Height) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, height)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, P>, height: Height) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, height)
    }
}

// ------- pointee impls ----------

macro_rules! impl_pointee {
    ($t:ident, $meta_ty:ty) => {
        impl<T, P: Ptr> Pointee for $t<T, P> {
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
        impl<T, P: Ptr> Borrow<$u<T, P>> for $t<T, P> {
            fn borrow(&self) -> &$u<T, P> {
                unsafe {
                    &*$u::make_fat_ptr(self as *const _ as *const (), self.height)
                }
            }
        }

        impl<T, P: Ptr> BorrowMut<$u<T, P>> for $t<T, P> {
            fn borrow_mut(&mut self) -> &mut $u<T, P> {
                unsafe {
                    &mut *$u::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
                }
            }
        }

        unsafe impl<T, P: Ptr> Take<$u<T, P>> for $t<T, P> {
            fn take_unsized<F, R>(self, f: F) -> R
                where F: FnOnce(RefOwn<$u<T, P>>) -> R
            {
                let mut this = ManuallyDrop::new(self);
                let this_dyn: &mut $u<T, P> = this.deref_mut().borrow_mut();

                unsafe {
                    f(RefOwn::new_unchecked(this_dyn))
                }
            }
        }

        impl<T, P: Ptr> IntoOwned for $u<T, P> {
            type Owned = $t<T, P>;

            fn into_owned(self: RefOwn<'_, Self>) -> Self::Owned {
                let this = RefOwn::leak(self);
                unsafe {
                    $t {
                        marker: PhantomData,
                        height: this.height(),
                        raw: ptr::read(&this.raw),
                    }
                }
            }
        }

        impl<T, P: Ptr> Deref for $t<T, P> {
            type Target = $u<T, P>;

            fn deref(&self) -> &Self::Target {
                self.borrow()
            }
        }

        impl<T, P: Ptr> DerefMut for $t<T, P> {
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

impl<T, P: Ptr> Blob for PerfectTree<T, P>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, P> as Blob>::SIZE + <Height as Blob>::SIZE;
    type DecodeBytesError = DecodePerfectTreeBytesError<<raw::Node<T, P> as Blob>::DecodeBytesError, <Height as Blob>::DecodeBytesError>;

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

impl<T, P: Ptr> Load for PerfectTree<T, P>
where T: Load
{
    type Blob = PerfectTree<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

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

unsafe impl<T, P: Ptr> BlobDyn for PerfectTreeDyn<T, P>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodePerfectTreeDynBytesError<<raw::Node<T, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<PerfectTree<T, P> as Blob>::SIZE)
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

impl<T, P: Ptr> LoadRef for PerfectTreeDyn<T, P>
where T: Load
{
    type BlobDyn = PerfectTreeDyn<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = PerfectTree::<T, P>::load_maybe_valid(blob, zone).trust();
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

impl<T, P: Ptr> Blob for Tip<T, P>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, P> as Blob>::SIZE + <NonZeroHeight as Blob>::SIZE;
    type DecodeBytesError = DecodeTipBytesError<<raw::Node<T, P> as Blob>::DecodeBytesError, <NonZeroHeight as Blob>::DecodeBytesError>;

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

impl<T, P: Ptr> Load for Tip<T, P>
where T: Load
{
    type Blob = Tip<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

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

unsafe impl<T, P: Ptr> BlobDyn for TipDyn<T, P>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodeTipDynBytesError<<raw::Node<T, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<Tip<T, P> as Blob>::SIZE)
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

impl<T, P: Ptr> LoadRef for TipDyn<T, P>
where T: Load
{
    type BlobDyn = TipDyn<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = Tip::<T, P>::load_maybe_valid(blob, zone).trust();
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

impl<T, P: Ptr> Blob for Pair<T, P>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <raw::Pair<T, P> as Blob>::SIZE + <NonZeroHeight as Blob>::SIZE;
    type DecodeBytesError = DecodePairBytesError<<raw::Pair<T, P> as Blob>::DecodeBytesError, <NonZeroHeight as Blob>::DecodeBytesError>;

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

impl<T, P: Ptr> Load for Pair<T, P>
where T: Load
{
    type Blob = Pair<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

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

unsafe impl<T, P: Ptr> BlobDyn for PairDyn<T, P>
where T: 'static,
      P: Blob,
{
    type DecodeBytesError = DecodePairDynBytesError<<raw::Pair<T, P> as Blob>::DecodeBytesError>;

    fn try_size(_height: Self::Metadata) -> Result<usize, !> {
        Ok(<Pair<T, P> as Blob>::SIZE)
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

impl<T, P: Ptr> LoadRef for PairDyn<T, P>
where T: Load
{
    type BlobDyn = PairDyn<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Self::Zone)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;

        let owned = Pair::<T, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

// -------- drop impls ------------
impl<T, P: Ptr> Drop for PerfectTreeDyn<T, P> {
    fn drop(&mut self) {
        match self.kind_mut() {
            Kind::Leaf(leaf) => unsafe { ptr::drop_in_place(leaf) },
            Kind::Tip(tip) => unsafe { ptr::drop_in_place(tip) },
        }
    }
}

impl<T, P: Ptr> Drop for PerfectTree<T, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, P: Ptr> Drop for TipDyn<T, P> {
    fn drop(&mut self) {
        let height = self.height();
        unsafe {
            self.raw.ptr.dealloc::<PairDyn<T, P>>(height);
        }
    }
}

impl<T, P: Ptr> Drop for Tip<T, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, P: Ptr> Drop for PairDyn<T, P> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, P: Ptr> Drop for Pair<T, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}


// -------------- fmt::Debug impls ---------------

impl<T, P: Ptr> fmt::Debug for PerfectTree<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, P: Ptr> fmt::Debug for PerfectTreeDyn<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, P: Ptr> TipDyn<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("digest", &self.raw.digest())
            .field("ptr", &self.try_get_dirty_pair()
                               .map_err(P::from_clean))
            .field("height", &self.height())
            .finish()
    }
}

impl<T, P: Ptr> fmt::Debug for Tip<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Tip", f)
    }
}

impl<T, P: Ptr> fmt::Debug for TipDyn<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("TipDyn", f)
    }
}

impl<T, P: Ptr> PairDyn<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &self.height())
            .finish()
    }
}

impl<T, P: Ptr> fmt::Debug for Pair<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Pair", f)
    }
}

impl<T, P: Ptr> fmt::Debug for PairDyn<T, P>
where T: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("PairDyn", f)
    }
}

/*
// --------- commit impls -----------

impl<T, P: Ptr> Commit for PerfectTreeDyn<T, P>
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

impl<T, P: Ptr> Commit for PerfectTree<T, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = PerfectTree<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}

impl<T, P: Ptr> Commit for TipDyn<T, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Tip<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.pair_digest().as_bytes());
        dst.write(&self.height());
    }
}

impl<T, P: Ptr> Commit for Tip<T, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Tip<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}

impl<T, P: Ptr> Commit for PairDyn<T, P>
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

impl<T, P: Ptr> Commit for Pair<T, P>
where T: Commit,
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN + 1;
    type Committed = Pair<T::Committed>;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        self.deref().encode_verbatim(dst)
    }
}
*/

// --------- save impls ------------

/*
#[doc(hidden)]
pub enum PerfectTreeDynSavePoll<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean> {
    //Leaf(Box<LeafSavePoll<T, P, S, R>>),
    Tip(Box<TipDynSavePoll<Q, R, T, P>>),
}

#[doc(hidden)]
pub struct PerfectTreeSavePoll<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean>(
    PerfectTreeDynSavePoll<Q, R, T, P>
);

#[doc(hidden)]
pub struct TipDynSavePoll<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean> {
    marker: PhantomData<fn(Q) -> T>,
    height: NonZeroHeight,
    digest: Digest,
    state: State<Q, R, T, P>,
}

enum State<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean> {
    Clean(P::Blob),
    Dirty(PairDynSavePoll<Q, R, T, P>),
    Done(R),
}


#[doc(hidden)]
pub struct PairDynSavePoll<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean> {
    left: PerfectTreeDynSavePoll<Q, R, T, P>,
    right: PerfectTreeDynSavePoll<Q, R, T, P>,
}
*/

/*
#[doc(hidden)]
pub enum PerfectTreeDynSavePoll<T, P: PtrClean, S, R> {
    Leaf(Box<LeafSavePoll<T, P, S, R>>),
    Tip(Box<TipDynSavePoll<T, P, S, R>>),
}

#[doc(hidden)]
pub struct PerfectTreeSavePoll<T, P: PtrClean, S, R>(
    PerfectTreeDynSavePoll<T, P, S, R>
);

#[doc(hidden)]
pub struct TipDynSavePoll<T, P: PtrClean, S, R> {
    marker: PhantomData<T>,
    height: NonZeroHeight,
    digest: Digest,
    state: State<T, P, S, R>,
}

enum State<T, P: PtrClean, S, R> {
    Clean(P::Blob),
    Dirty(PairDynSavePoll<T, P, S, R>),
    Done(R),
}


#[doc(hidden)]
pub struct PairDynSavePoll<T, P: PtrClean, S, R> {
    left: PerfectTreeDynSavePoll<T, P, S, R>,
    right: PerfectTreeDynSavePoll<T, P, S, R>,
}
*/

/*
impl<T: SavePoll, P: PtrConst> PerfectTreeDynSavePoll<T, P> {
    fn encode_raw_node_blob(&self) -> raw::Node<T::SavedBlob, (), P::Blob> {
        match self {
            Self::Leaf(leaf) => leaf.encode_raw_node_blob(),
            Self::Tip(tip) => tip.encode_raw_node_blob(),
        }
    }
}

impl<T: SavePoll, P: PtrConst> TipDynSavePoll<T, P> {
    fn encode_raw_node_blob(&self) -> raw::Node<T::SavedBlob, (), P::Blob> {
        match self.state {
            State::Done(ptr) => raw::Node::new(Some(self.digest), (), ptr),
            State::(_) => panic!(),
        }
    }
}

impl<T: SavePoll, P: PtrConst> PairDynSavePoll<T, P> {
    fn encode_raw_pair_blob(&self) -> raw::Pair<T::SavedBlob, (), P::Blob> {
        raw::Pair {
            left: self.left.encode_raw_node_blob(),
            right: self.right.encode_raw_node_blob(),
        }
    }
}
*/

/*
impl<Q, R: PtrBlob, T: Save<Q, R>, P: PtrClean> SaveRefPoll<Q, R> for PairDynSavePoll<Q, R, T, P>
{
    type DstBlob = PairDyn<T::DstBlob, R>;

    fn blob_metadata(&self) -> NonZeroHeight {
        self.left.blob_metadata()
                 .try_increment().expect("valid metadata")
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
    {
        /*
        self.left.save_dirty_ref_poll_impl(saver)?;
        self.right.save_dirty_ref_poll_impl(saver)
        */ todo!()
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        /*
        dst.write_struct()
           .write_field(&self.encode_raw_pair_blob())
           .done()
        */ todo!()
    }
}
*/

/*
impl<Q, R: PtrBlob, T, P: Ptr> SaveRefPoll<Q, R> for PerfectTreeDynSavePoll<T, P, T::SavePoll, R>
where T: Save<Q, R>
{
    type DstBlob = PerfectTreeDyn<T::DstBlob, R>;

    fn blob_metadata(&self) -> Height {
        match self {
            Self::Leaf(_) => Height::ZERO,
            Self::Tip(tip) => tip.blob_metadata().into(),
        }
    }

    fn save_ref_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
    {
        /*
        match self {
            Self::Leaf(leaf) => leaf.save_dirty_ref_poll_impl(saver),
            Self::Tip(tip) => tip.save_dirty_ref_poll_impl(saver),
        }
        */ todo!()
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::DstBlob>) -> Bytes<'a, Self::DstBlob> {
        /*
        dst.write_struct()
           .write_field(&self.encode_raw_node_blob())
           .done()
        */ todo!()
    }
}
*/

/*
impl<T: SavePoll, P: PtrConst> SaveRefPoll for TipDynSavePoll<T, P>
where T::CleanPtr: FromPtr<P>
{
    type CleanPtr = P;
    type SavedBlobDyn = TipDyn<T::SavedBlob, (), P::Blob>;

    fn blob_metadata(&self) -> NonZeroHeight {
        self.height
    }

    fn save_dirty_ref_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>
    {
        loop {
            self.state = match &mut self.state {
                State::(dirty) => {
                    dirty.save_dirty_ref_poll_impl(saver)?;

                    let q_blob = saver.save_bytes(self.height, |dst| {
                        dirty.encode_blob_dyn_bytes(dst)
                    })?;

                    State::Done(q_blob)
                },
                State::Done(_) => break Ok(()),
            };
        }
    }

    fn encode_blob_dyn_bytes<'a>(&self, dst: BytesUninit<'a, Self::SavedBlobDyn>) -> Bytes<'a, Self::SavedBlobDyn> {
        dst.write_struct()
           .write_field(&self.encode_raw_node_blob())
           .done()
    }
}
*/

/*
impl<Q, R: PtrBlob, T, P: Ptr> SaveRef<Q, R> for PerfectTreeDyn<T, P>
where T: Save<Q, R>
{
    type SrcBlob = PerfectTreeDyn<T::SrcBlob, P::Blob>;
    type DstBlob = PerfectTreeDyn<T::DstBlob, R>;
    type SavePoll = PerfectTreeDynSavePoll<T, P::Clean, T::SavePoll, R>;

    fn init_save_ref(&self) -> Self::SavePoll {
        /*
        match self.kind() {
            Kind::Leaf(leaf) => PerfectTreeDynSavePoll::Leaf(leaf.init_save_dirty().into()),
            Kind::Tip(tip) => PerfectTreeDynSavePoll::Tip(tip.init_save_dirty_ref().into()),
        }
        */ todo!()
    }

    fn init_save_ref_from_bytes(_: Bytes<'_, Self::SrcBlob>)
        -> Result<Self::SavePoll,
                  <Self::SrcBlob as BlobDyn>::DecodeBytesError>
    {
        todo!()
    }
}
*/

/*
impl<T, Z: Zone, P: Ptr> SaveRef for TipDyn<T, P>
where T: Commit + Save,
      T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveRefPoll = TipDynSavePoll<T::SavePoll, P::Clean>;

    fn init_save_dirty_ref(&self) -> Self::SaveRefPoll {
        TipDynSavePoll {
            height: self.height(),
            digest: Digest::default(),
            state: match self.try_get_dirty_pair() {
                Ok(pair) => State::(pair.init_save_dirty_ref()),
                Err(p_clean) => State::Done(p_clean.to_blob()),
            }
        }
    }
}

impl<T, Z: Zone, P: Ptr> SaveRef for PairDyn<T, P>
where T: Commit + Save,
      T::CleanPtr: FromPtr<P::Clean>
{
    type CleanPtr = P::Clean;
    type SaveRefPoll = PairDynSavePoll<T::SavePoll, P::Clean>;

    fn init_save_dirty_ref(&self) -> Self::SaveRefPoll {
        PairDynSavePoll {
            left: self.left().init_save_dirty_ref(),
            right: self.right().init_save_dirty_ref(),
        }
    }
}
*/

/*
impl<Q, R: PtrBlob, T, P: PtrClean> SavePoll<Q, R> for PerfectTreeSavePoll<T, P, T::SavePoll, R>
where T: Save<Q, R>
{
    type DstBlob = PerfectTree<T::DstBlob, R>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
    {
        //self.0.save_ref_poll_impl(saver)
        todo!()
    }

    fn encode_blob(&self) -> Self::DstBlob {
        /*
        let raw = self.0.encode_raw_node_blob();
        let height = self.0.blob_metadata();

        unsafe {
            PerfectTree::from_raw_node(raw, height)
        }
        */ todo!()
    }
}

impl<Q, R: PtrBlob, T, P: Ptr> Save<Q, R> for PerfectTree<T, P>
where T: Save<Q, R>
{
    type SrcBlob = PerfectTree<T::SrcBlob, P::Blob>;
    type DstBlob = PerfectTree<T::DstBlob, R>;
    type SavePoll = PerfectTreeSavePoll<T, P::Clean, T::SavePoll, R>;

    fn init_save(&self) -> Self::SavePoll {
        /*
        PerfectTreeSavePoll(
            self.deref().init_save_dirty_ref()
        )
        */ todo!()
    }

    fn init_save_from_blob(blob: &Self::SrcBlob) -> Self::SavePoll {
        todo!()
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::ptr::Heap;

    #[test]
    fn test_get() {
        let leaf0 = PerfectTree::<u8, Heap>::new_leaf(0u8);
        let leaf1 = PerfectTree::<u8, Heap>::new_leaf(1u8);
        let tree0 = PerfectTree::try_join(leaf0, leaf1).unwrap();
        assert_eq!(tree0.get(0).unwrap(), &0);
        assert_eq!(tree0.get(1).unwrap(), &1);
        assert_eq!(tree0.get(2), None);
        assert_eq!(tree0.get(usize::MAX), None);
    }

    #[test]
    fn test_commit() {
        /*
        let n = 0u8;
        let _d_n = n.commit();

        let leaf0 = PerfectTree::new_leaf_in(0u8, Heap);
        let leaf1 = PerfectTree::new_leaf_in(1u8, Heap);
        let tree0 = PerfectTree::try_join(leaf0, leaf1).unwrap();

        let _ = tree0.commit();
        let _ = tree0.to_verbatim();
        */
    }
}
