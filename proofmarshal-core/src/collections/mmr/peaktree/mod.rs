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

use crate::commit::{Commit, WriteVerbatim, Digest};
use crate::unreachable_unchecked;

use crate::collections::{
    height::*,
    length::*,
    raw,
    perfecttree::{PerfectTree, PerfectTreeDyn},
};

#[repr(C)]
pub struct Pair<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, Z, P>>,
    len: InnerLength,
}

#[repr(C)]
pub struct PairDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Pair<T, Z, P>>,
    len: InnerLengthDyn,
}

#[repr(C)]
pub struct Inner<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    len: InnerLength,
}

#[repr(C)]
pub struct InnerDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    len: InnerLengthDyn,
}

#[repr(C)]
pub struct PeakTree<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    len: NonZeroLength,
}

#[repr(C)]
pub struct PeakTreeDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
    len: NonZeroLengthDyn,
}

#[derive(Debug)]
pub enum Kind<Peak, Inner> {
    Peak(Peak),
    Inner(Inner),
}

impl<T, Z, P: Ptr> From<PerfectTree<T, Z, P>> for PeakTree<T, Z, P> {
    fn from(peak: PerfectTree<T, Z, P>) -> Self {
        let len = NonZeroLength::from_height(peak.height());
        let raw = peak.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, Z, P: Ptr> From<Inner<T, Z, P>> for PeakTree<T, Z, P> {
    fn from(inner: Inner<T, Z, P>) -> Self {
        let len = inner.len().into();
        let raw = inner.into_raw_node();
        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, Z: Zone> PeakTree<T, Z>
where T: Load
{
    pub(crate) fn try_push_peak(self, peak: PerfectTree<T, Z>) -> Result<Self, (Self, PerfectTree<T, Z>)>
        where Z: Alloc + GetMut
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

    fn merge_peak(self, right: PerfectTree<T, Z>) -> PerfectTree<T, Z>
        where Z: Alloc + GetMut
    {
        match self.into_kind() {
            Kind::Inner(inner) => inner.merge_peak(right),
            Kind::Peak(left) => PerfectTree::try_join(left, right).ok().expect("overflow"),
        }
    }
}

impl<T, Z, P: Ptr> PeakTree<T, Z, P> {
    pub fn into_kind(self) -> Kind<PerfectTree<T, Z, P>, Inner<T, Z, P>> {
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

impl<T, Z: Zone> PeakTreeDyn<T, Z>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, Z>>>
        where Z: Get
    {
        match self.kind() {
            Kind::Peak(peak) if peak.height() == height => Some(Ref::Borrowed(peak)),
            Kind::Peak(_) => None,
            Kind::Inner(inner) => inner.get(height),
        }
    }
}

impl<T, Z, P: Ptr> PeakTreeDyn<T, Z, P> {
    pub fn len(&self) -> NonZeroLength {
        self.len.to_nonzero_length()
    }

    pub fn kind(&self) -> Kind<&PerfectTreeDyn<T, Z, P>, &InnerDyn<T, Z, P>> {
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

    pub fn kind_mut(&mut self) -> Kind<&mut PerfectTreeDyn<T, Z, P>, &mut InnerDyn<T, Z, P>> {
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

    /*
    pub fn node_digest(&self) -> Digest
        where T: Commit
    {
        match self.kind() {
            Kind::Peak(peak) => peak.digest().cast(),
            Kind::Inner(inner) => inner.pair_digest().cast(),
        }
    }

    pub fn try_node_digest(&self) -> Option<Digest>
    {
        match self.kind() {
            Kind::Peak(peak) => peak.try_digest(),
            Kind::Inner(inner) => inner.try_pair_digest(),
        }
    }

    pub fn zone(&self) -> Z
        where Z: Copy
    {
        self.raw.zone
    }
    */
}

impl<T, Z: Zone> Inner<T, Z> {
    pub fn try_join_peaks(left: PerfectTree<T, Z>, right: PerfectTree<T, Z>)
        -> Result<Self, (PerfectTree<T, Z>, PerfectTree<T, Z>)>
        where Z: Alloc
    {
        let pair = Pair::try_join_peaks(left, right)?;
        Ok(Self::new(pair))
    }

    pub fn new(pair: Pair<T, Z>) -> Self
        where Z: Alloc
    {
        let zone = pair.zone();
        Self::new_unchecked(None, zone.alloc(pair))
    }

    pub fn new_unchecked(digest: Option<Digest>, pair: Bag<PairDyn<T, Z>, Z>) -> Self {
        let (ptr, len, zone) = pair.into_raw_parts();
        let raw = raw::Node::new(digest, zone, ptr);

        unsafe {
            Self::from_raw_node(raw, len)
        }
    }
}

impl<T, Z: Zone> Inner<T, Z>
where T: Load
{
    fn merge_peak(self, right: PerfectTree<T, Z>) -> PerfectTree<T, Z>
        where Z: Alloc + GetMut
    {
        let pair = self.into_pair();
        pair.merge_peak(right)
    }

    pub(crate) fn try_push_peak(self, peak: PerfectTree<T, Z>) -> Result<PeakTree<T, Z>, (Self, PerfectTree<T, Z>)>
        where Z: Alloc + GetMut
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

    /*
    pub(crate) fn try_push_peak_impl(self, right: PerfectTree<T, Z>) -> Result<Self, Option<PeakTree<T, Z>>>
        where Z: Alloc + GetMut
    {
        todo!()
        /*
        match self.len().push_peak(right.height()) {
            Ok(new_len) => {
                let (new_left, new_right) = new_len.split();
                if new_left == self.len() {
                } else {
                }
            },
            Err(Some(height)) => {
                todo!()
            },
            Err(None) => Err(None),
        }
        */

        /*
        if self.len().min_height() > right.height() {
            let pair = Pair::new(self.into(), right.into());
            Ok(Self::new(pair))
        } else {
            let pair = self.into_pair();
            let pair = pair.try_push_peak_impl(right)?;
            Ok(Self::new(pair))
        }
        */
    }
    */

    pub fn into_pair(self) -> Pair<T, Z>
        where Z: Get
    {
        let len = self.len();
        let raw = self.into_raw_node();
        unsafe {
            raw.take_unchecked::<PairDyn<T, Z>>(len)
               .trust()
        }
    }
}

impl<T, Z: Zone> InnerDyn<T, Z>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, Z>>>
        where Z: Get
    {
        if self.len().contains(height) {
            match self.get_pair() {
                Ref::Borrowed(pair) => pair.get(height),
                Ref::Owned(_pair) => todo!()
            }
        } else {
            None
        }
    }

    pub fn get_pair(&self) -> Ref<PairDyn<T, Z>>
        where Z: Get
    {
        unsafe {
            self.raw.get_unchecked(self.len())
                    .trust()
        }
    }

    /*
    pub fn get_pair_mut(&mut self) -> &mut PairDyn<T, Z>
        where Z: GetMut
    {
        let len = self.len();
        unsafe {
            self.raw.get_unchecked_mut(len)
                    .trust()
        }
    }
    */
}

impl<T, Z, P: Ptr> InnerDyn<T, Z, P> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
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
                       .ok().expect("digest missing yet inner ptr clean");
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
    */
}

impl<T, Z: Zone> Pair<T, Z>
where T: Load
{
    fn merge_peak(self, peak: PerfectTree<T, Z>) -> PerfectTree<T, Z>
        where Z: Alloc + GetMut
    {
        let (left, right) = self.into_split();
        let peak = right.merge_peak(peak);
        left.merge_peak(peak)
    }
}


impl<T, Z, P: Ptr> Pair<T, Z, P>
{
    pub fn try_join_peaks(left: PerfectTree<T, Z, P>, right: PerfectTree<T, Z, P>)
        -> Result<Self, (PerfectTree<T, Z, P>, PerfectTree<T, Z, P>)>
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

    pub fn new(left: PeakTree<T, Z, P>, right: PeakTree<T, Z, P>) -> Self {
        assert!(left.len().min_height() > right.len().max_height());
        let len = left.len().get() | right.len().get();
        let len = InnerLength::try_from(len.get()).unwrap();

        assert_eq!(len.split(), (left.len(), right.len()));

        unsafe {
            Self::new_unchecked(left.into_raw_node(), right.into_raw_node(), len)
        }
    }

    pub unsafe fn new_unchecked(left: raw::Node<T, Z, P>, right: raw::Node<T, Z, P>, len: InnerLength) -> Self {
        Self::from_raw_pair(
            raw::Pair { left, right },
            len
        )
    }

    pub fn into_split(self) -> (PeakTree<T, Z, P>, PeakTree<T, Z, P>) {
        let (left_len, right_len) = self.len().split();
        let raw = self.into_raw_pair();

        unsafe {
            (PeakTree::from_raw_node(raw.left, left_len),
             PeakTree::from_raw_node(raw.right, right_len))
        }
    }
}

impl<T, Z: Zone> PairDyn<T, Z>
where T: Load
{
    pub fn get(&self, height: Height) -> Option<Ref<PerfectTreeDyn<T, Z>>>
        where Z: Get
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

impl<T, Z, P: Ptr> PairDyn<T, Z, P> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub fn zone(&self) -> Z
        where Z: Copy
    {
        self.raw.left.zone
    }

    pub fn split(&self) -> (&PeakTreeDyn<T, Z, P>, &PeakTreeDyn<T, Z, P>) {
        let (left_len, right_len) = self.len().split();
        unsafe {
            (PeakTreeDyn::from_raw_node_ref(&self.raw.left, left_len),
             PeakTreeDyn::from_raw_node_ref(&self.raw.right, right_len))
        }
    }

    pub fn split_mut(&mut self) -> (&mut PeakTreeDyn<T, Z, P>, &mut PeakTreeDyn<T, Z, P>) {
        let (left_len, right_len) = self.len().split();
        let (left, right) = self.raw.split_mut();
        unsafe {
            (PeakTreeDyn::from_raw_node_mut(left, left_len),
             PeakTreeDyn::from_raw_node_mut(right, right_len))
        }
    }

    pub fn left(&self) -> &PeakTreeDyn<T, Z, P> {
        self.split().0
    }

    pub fn left_mut(&mut self) -> &mut PeakTreeDyn<T, Z, P> {
        self.split_mut().0
    }

    pub fn right(&self) -> &PeakTreeDyn<T, Z, P> {
        self.split().1
    }

    pub fn right_mut(&mut self) -> &mut PeakTreeDyn<T, Z, P> {
        self.split_mut().1
    }

}

// --------- conversions from raw -------------

impl<T, Z, P: Ptr> Pair<T, Z, P> {
    pub unsafe fn from_raw_pair(raw: raw::Pair<T, Z, P>, len: InnerLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_pair(self) -> raw::Pair<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> PairDyn<T, Z, P> {
    pub unsafe fn from_raw_pair_ref(raw: &raw::Pair<T, Z, P>, len: InnerLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_pair_mut(raw: &mut raw::Pair<T, Z, P>, len: InnerLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
    }
}

impl<T, Z, P: Ptr> Inner<T, Z, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, Z, P>, len: InnerLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> InnerDyn<T, Z, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, Z, P>, len: InnerLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, Z, P>, len: InnerLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
    }

    pub fn try_get_dirty_pair(&self) -> Result<&PairDyn<T, Z, P>, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(self.len())
        }
    }
}

impl<T, Z, P: Ptr> PeakTree<T, Z, P> {
    pub unsafe fn from_raw_node(raw: raw::Node<T, Z, P>, len: NonZeroLength) -> Self {
        Self {
            marker: PhantomData,
            raw: ManuallyDrop::new(raw),
            len,
        }
    }

    pub fn into_raw_node(self) -> raw::Node<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&*this.raw) }
    }
}

impl<T, Z, P: Ptr> PeakTreeDyn<T, Z, P> {
    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, Z, P>, len: NonZeroLength) -> &Self {
        &*Self::make_fat_ptr(raw as *const _ as *const _, len)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, Z, P>, len: NonZeroLength) -> &mut Self {
        &mut *Self::make_fat_ptr_mut(raw as *mut _ as *mut _, len)
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
        impl<T, Z, P: Ptr> Borrow<$u<T, Z, P>> for $t<T, Z, P> {
            fn borrow(&self) -> &$u<T, Z, P> {
                unsafe {
                    &*$u::make_fat_ptr(self as *const _ as *const (), self.len)
                }
            }
        }

        impl<T, Z, P: Ptr> BorrowMut<$u<T, Z, P>> for $t<T, Z, P> {
            fn borrow_mut(&mut self) -> &mut $u<T, Z, P> {
                unsafe {
                    &mut *$u::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
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
                        len: this.len(),
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

impl_deref!(PeakTree => PeakTreeDyn);
impl_deref!(Inner => InnerDyn);
impl_deref!(Pair => PairDyn);

// ------- hoard impls ----------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePeakTreeBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, Z, P: Ptr> Blob for PeakTree<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, Z, P> as Blob>::SIZE + <NonZeroLength as Blob>::SIZE;
    type DecodeBytesError = DecodePeakTreeBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError, <NonZeroLength as Blob>::DecodeBytesError>;

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

impl<T, Z: Zone, P: Ptr> Load for PeakTree<T, Z, P>
where T: Load
{
    type Blob = PeakTree<T::Blob, (), P::Blob>;
    type Zone = Z;

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
pub struct DecodePeakTreeDynBytesError<Raw: error::Error>(Raw);

unsafe impl<T, Z, P: Ptr> BlobDyn for PeakTreeDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodePeakTreeDynBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<PeakTree<T, Z, P> as Blob>::SIZE)
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

impl<T, Z: Zone, P: Ptr> LoadRef for PeakTreeDyn<T, Z, P>
where T: Load
{
    type BlobDyn = PeakTreeDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = PeakTree::<T, Z, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeInnerBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, Z, P: Ptr> Blob for Inner<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Node<T, Z, P> as Blob>::SIZE + <InnerLength as Blob>::SIZE;
    type DecodeBytesError = DecodeInnerBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError, <InnerLength as Blob>::DecodeBytesError>;

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

impl<T, Z: Zone, P: Ptr> Load for Inner<T, Z, P>
where T: Load
{
    type Blob = Inner<T::Blob, (), P::Blob>;
    type Zone = Z;

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

unsafe impl<T, Z, P: Ptr> BlobDyn for InnerDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodeInnerDynBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<Inner<T, Z, P> as Blob>::SIZE)
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

impl<T, Z: Zone, P: Ptr> LoadRef for InnerDyn<T, Z, P>
where T: Load
{
    type BlobDyn = InnerDyn<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(src: Bytes<'a, Self::BlobDyn>, zone: &Z)
        -> Result<MaybeValid<Ref<'a, Self>>, <Self::BlobDyn as BlobDyn>::DecodeBytesError>
    {
        let blob = <Self::BlobDyn as BlobDyn>::decode_bytes(src)?;
        let owned = Inner::<T, Z, P>::load_maybe_valid(blob, zone).trust();
        Ok(MaybeValid::from(Ref::Owned(owned)))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodePairBytesError<Raw: error::Error, NonZeroLength: error::Error> {
    Raw(Raw),
    NonZeroLength(NonZeroLength),
}

impl<T, Z, P: Ptr> Blob for Pair<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <raw::Pair<T, Z, P> as Blob>::SIZE + <InnerLength as Blob>::SIZE;
    type DecodeBytesError = DecodePairBytesError<<raw::Pair<T, Z, P> as Blob>::DecodeBytesError, <InnerLength as Blob>::DecodeBytesError>;

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

impl<T, Z: Zone, P: Ptr> Load for Pair<T, Z, P>
where T: Load
{
    type Blob = Pair<T::Blob, (), P::Blob>;
    type Zone = Z;

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

unsafe impl<T, Z, P: Ptr> BlobDyn for PairDyn<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    type DecodeBytesError = DecodePairDynBytesError<<raw::Pair<T, Z, P> as Blob>::DecodeBytesError>;

    fn try_size(_len: Self::Metadata) -> Result<usize, !> {
        Ok(<Pair<T, Z, P> as Blob>::SIZE)
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
impl<T, Z, P: Ptr> Drop for PeakTreeDyn<T, Z, P> {
    fn drop(&mut self) {
        match self.kind_mut() {
            Kind::Peak(peak) => unsafe { ptr::drop_in_place(peak) },
            Kind::Inner(inner) => unsafe { ptr::drop_in_place(inner) },
        }
    }
}

impl<T, Z, P: Ptr> Drop for PeakTree<T, Z, P> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.deref_mut()) }
    }
}

impl<T, Z, P: Ptr> Drop for InnerDyn<T, Z, P> {
    fn drop(&mut self) {
        let len = self.len();
        unsafe {
            self.raw.ptr.dealloc::<PairDyn<T, Z, P>>(len);
        }
    }
}

impl<T, Z, P: Ptr> Drop for Inner<T, Z, P> {
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

impl<T, Z, P: Ptr> fmt::Debug for PeakTree<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for PeakTreeDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<T, Z, P: Ptr> InnerDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("digest", &self.raw.digest())
            .field("zone", &self.raw.zone)
            .field("ptr", &self.try_get_dirty_pair()
                               .map_err(P::from_clean))
            .field("len", &self.len())
            .finish()
    }
}

impl<T, Z, P: Ptr> fmt::Debug for Inner<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("Inner", f)
    }
}

impl<T, Z, P: Ptr> fmt::Debug for InnerDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_debug_impl("InnerDyn", f)
    }
}

impl<T, Z, P: Ptr> PairDyn<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug,
{
    fn fmt_debug_impl(&self, name: &'static str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("left", &self.left())
            .field("right", &self.right())
            .field("len", &self.len())
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
    fn test() {
        let peak = PerfectTree::new_leaf_in(42u8, Heap);
        let _peaks = PeakTree::from(peak);
    }
}
