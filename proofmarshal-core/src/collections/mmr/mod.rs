use std::cell::Cell;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::error;
use std::borrow::{Borrow, BorrowMut};
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::DerefMut;
use std::ptr;

use thiserror::Error;

use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::primitive::Primitive;
use hoard::owned::{IntoOwned, Take, Ref, Own};
use hoard::pointee::Pointee;
use hoard::zone::{Alloc, Get, GetMut, Ptr, PtrBlob, Zone};
use hoard::load::{Load, LoadRef, MaybeValid};

use crate::commit::Digest;
use crate::collections::perfecttree::height::*;
use crate::collections::perfecttree::{SumPerfectTree, SumPerfectTreeDyn};
use crate::collections::merklesum::MerkleSum;

pub mod length;
use self::length::*;

#[derive(Debug)]
pub struct SumMMR<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr, L: ?Sized + ToLength = Length> {
    marker: PhantomData<T>,
    zone: Z,
    tip_ptr: MaybeUninit<P>,
    tip_digest: Cell<Option<Digest>>,
    sum: Cell<Option<S>>,
    len: L,
}

pub type SumMMRDyn<T, S, Z, P = <Z as Zone>::Ptr> = SumMMR<T, S, Z, P, LengthDyn>;

#[derive(Debug)]
pub struct Inner<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr, L: ?Sized + ToInnerLength = InnerLength> {
    peak: SumPerfectTree<T, S, Z, P, DummyHeight>,
    next: SumMMR<T, S, Z, P, DummyNonZeroLength>,
    len: L,
}

pub type InnerDyn<T, S, Z, P = <Z as Zone>::Ptr> = Inner<T, S, Z, P, InnerLengthDyn>;

pub enum Tip<Peak, Inner> {
    Empty,
    Peak(Peak),
    Inner(Inner),
}

impl<T, S: Copy, Z, P: Ptr> SumMMR<T, S, Z, P> {
    pub fn new_in(zone: Z) -> Self
        where S: Default
    {
        unsafe {
            Self::from_raw_parts(
                zone,
                None,
                Some(Digest::default()),
                Some(S::default()),
                0.into(),
            )
        }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum PushError<SumError: error::Error, ZoneError: error::Error> {
    HeightOverflow,
    SumOverflow(SumError),
    Zone(ZoneError),
}

impl<S: error::Error, Z: error::Error> From<Z> for PushError<S, Z> {
    fn from(err: Z) -> Self {
        PushError::Zone(err)
    }
}

impl<T, S: Copy, Z: Zone> SumMMR<T, S, Z>
where T: Load,
      S: Blob + Default
{
    pub fn push(&mut self, value: T) -> Result<(), Z::Error>
        where Z: Alloc + GetMut
    {
        match self.take_tip()? {
            Tip::Empty => {
                let (ptr, (), _zone) = self.zone.alloc(value).into_raw_parts();
                self.tip_ptr = MaybeUninit::new(ptr);
                self.len = Length(1);
            },
            Tip::Peak(peak) => {
                todo!()
            },
            Tip::Inner(inner) => {
                todo!()
            },
        };
        Ok(())
    }

    pub fn take_tip(&mut self) -> Result<Tip<SumPerfectTree<T, S, Z>, Inner<T, S, Z>>, Z::Error>
        where Z: Get
    {
        if let Ok(len) = NonZeroLength::try_from(self.len()) {
            self.sum.set(Some(S::default()));
            self.tip_digest.set(None);
            self.len = Length(0);
            let tip_ptr = unsafe { self.tip_ptr.as_ptr().read() };

            match len.try_into_inner_length() {
                Ok(len) => {
                    let inner = unsafe {
                        self.zone.take_unchecked::<InnerDyn<T, S, Z>>(tip_ptr, len)?
                    };
                    Ok(Tip::Inner(inner.trust()))
                },
                Err(height) => {
                    let peak = unsafe {
                        self.zone.take_unchecked::<SumPerfectTreeDyn<T, S, Z>>(tip_ptr, height)?
                    };
                    Ok(Tip::Peak(peak.trust()))
                },
            }
        } else {
            Ok(Tip::Empty)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> Default for SumMMR<T, S, Z, P>
where S: Default,
      Z: Default
{
    fn default() -> Self {
        Self::new_in(Z::default())
    }
}

impl<T, S: Copy, Z, P: Ptr, L: ToLength> SumMMR<T, S, Z, P, L> {
    pub unsafe fn from_raw_parts(
        zone: Z,
        tip_ptr: Option<P>,
        tip_digest: Option<Digest>,
        sum: Option<S>,
        len: L,
    ) -> Self {
        Self {
            marker: PhantomData,
            zone,
            tip_ptr: tip_ptr.map(MaybeUninit::new).unwrap_or(MaybeUninit::uninit()),
            tip_digest: tip_digest.into(),
            sum: sum.into(),
            len,
        }
    }

    pub fn into_raw_parts(self) -> (Z, Option<P>, Option<Digest>, Option<S>, L) {
        todo!()
    }
}

impl<T, S: Copy, Z, P: Ptr, L: ?Sized + ToLength> SumMMR<T, S, Z, P, L> {
    pub fn len(&self) -> usize {
        self.len.to_length().into()
    }

    fn tip_ptr(&self) -> Option<&P> {
        if self.len() > 0 {
            unsafe {
                Some(&*self.tip_ptr.as_ptr())
            }
        } else {
            None
        }
    }

    fn tip_ptr_mut(&mut self) -> Option<&mut P> {
        if self.len() > 0 {
            unsafe {
                Some(&mut *self.tip_ptr.as_mut_ptr())
            }
        } else {
            None
        }
    }

    pub fn try_get_dirty_tip(&self) -> Result<Tip<&SumPerfectTreeDyn<T, S, Z, P>,
                                                  &InnerDyn<T, S, Z, P>>,
                                              P::Clean>
    {
        if let Ok(len) = NonZeroLength::try_from(self.len()) {
            let tip_ptr = self.tip_ptr().unwrap();
            match len.try_into_inner_length() {
                Ok(len) => {
                    let inner = unsafe { tip_ptr.try_get_dirty(len)? };
                    Ok(Tip::Inner(inner))
                },
                Err(height) => {
                    let peak = unsafe { tip_ptr.try_get_dirty(height)? };
                    Ok(Tip::Peak(peak))
                },
            }
        } else {
            Ok(Tip::Empty)
        }
    }

    pub fn try_get_dirty_tip_mut(&mut self) -> Result<Tip<&mut SumPerfectTreeDyn<T, S, Z, P>,
                                                          &mut InnerDyn<T, S, Z, P>>,
                                                      P::Clean>
    {
        if let Ok(len) = NonZeroLength::try_from(self.len()) {
            let tip_ptr = self.tip_ptr_mut().unwrap();
            match len.try_into_inner_length() {
                Ok(len) => {
                    let inner = unsafe { tip_ptr.try_get_dirty_mut(len)? };
                    Ok(Tip::Inner(inner))
                },
                Err(height) => {
                    let peak = unsafe { tip_ptr.try_get_dirty_mut(height)? };
                    Ok(Tip::Peak(peak))
                },
            }
        } else {
            Ok(Tip::Empty)
        }
    }
}


pub enum InnerJoinError<ZoneError> {
    Zone(ZoneError),
    HeightOverflow,
    SumOverflow,
}

impl<ZoneError> From<ZoneError> for InnerJoinError<ZoneError> {
    fn from(err: ZoneError) -> Self {
        InnerJoinError::Zone(err)
    }
}

impl<T, S: Copy, Z: Zone> Inner<T, S, Z>
where T: Load,
      S: Blob + Default
{
    pub fn try_join_in(
        peak: SumPerfectTree<T, S, Z>,
        mut next: SumMMR<T, S, Z>,
    ) -> Result<Self, InnerJoinError<Z::Error>>
        where Z: Alloc + GetMut
    {
        let zone: Z = next.zone;
        if peak.len() == next.len() {
            assert!(peak.len().is_power_of_two());
            if let Tip::Peak(next_peak) = next.take_tip()? {
                next_peak.try_join(peak);
                todo!()
            } else {
                unreachable!()
            }
        } else {
            todo!()
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, L: ToInnerLength> Inner<T, S, Z, P, L> {
    pub unsafe fn new_unchecked<HP, HN>(
        peak: SumPerfectTree<T, S, Z, P, HP>,
        next: SumMMR<T, S, Z, P, HN>,
        len: L
    ) -> Self
        where HP: ToHeight,
              HN: ToLength,
    {
        todo!()
    }
}

impl<T, S: Copy, Z, P: Ptr, L: ?Sized + ToInnerLength> Inner<T, S, Z, P, L> {
    pub fn len(&self) -> usize {
        self.len.to_length().into()
    }

    pub fn peak(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        let (height, _) = self.len.to_inner_length().split();
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(&self.peak as *const _ as *const _, height)
        }
    }

    pub fn peak_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        let (height, _) = self.len.to_inner_length().split();
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(&mut self.peak as *mut _ as *mut _, height)
        }
    }

    pub fn next(&self) -> &SumMMRDyn<T, S, Z, P> {
        let (_, next_len) = self.len.to_inner_length().split();
        unsafe {
            &*SumMMR::make_fat_ptr(&self.next as *const _ as *const _, next_len.into())
        }
    }

    pub fn next_mut(&mut self) -> &mut SumMMRDyn<T, S, Z, P> {
        let (_, next_len) = self.len.to_inner_length().split();
        unsafe {
            &mut *SumMMR::make_fat_ptr_mut(&mut self.next as *mut _ as *mut _, next_len.into())
        }
    }
}

// ------- unsizing related impls ------------

impl<T, S: Copy, Z, P: Ptr> Pointee for SumMMRDyn<T, S, Z, P> {
    type Metadata = Length;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            ptr.len().into()
        }
    }

    fn make_fat_ptr(thin: *const (), length: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, length.into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), length: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, length.into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S: Copy, Z, P: Ptr> Pointee for InnerDyn<T, S, Z, P> {
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

    fn make_fat_ptr(thin: *const (), length: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, length.into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), length: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, length.into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S: Copy, Z, P: Ptr> Borrow<SumMMRDyn<T, S, Z, P>> for SumMMR<T, S, Z, P> {
    fn borrow(&self) -> &SumMMRDyn<T, S, Z, P> {
        unsafe {
            &*SumMMRDyn::make_fat_ptr(self as *const _ as *const (), self.len)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> BorrowMut<SumMMRDyn<T, S, Z, P>> for SumMMR<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut SumMMRDyn<T, S, Z, P> {
        unsafe {
            &mut *SumMMRDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
        }
    }
}

unsafe impl<T, S: Copy, Z, P: Ptr> Take<SumMMRDyn<T, S, Z, P>> for SumMMR<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<SumMMRDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> IntoOwned for SumMMRDyn<T, S, Z, P> {
    type Owned = SumMMR<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            SumMMR {
                marker: PhantomData,
                zone: ptr::read(&this.zone),
                tip_ptr: ptr::read(&this.tip_ptr),
                tip_digest: ptr::read(&this.tip_digest),
                sum: ptr::read(&this.sum),
                len: this.len.to_length(),
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> Borrow<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow(&self) -> &InnerDyn<T, S, Z, P> {
        unsafe {
            &*InnerDyn::make_fat_ptr(self as *const _ as *const (), self.len)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> BorrowMut<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.len)
        }
    }
}

unsafe impl<T, S: Copy, Z, P: Ptr> Take<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<InnerDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn = this.deref_mut().borrow_mut();

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
                peak: ptr::read(&this.peak),
                next: ptr::read(&this.next),
                len: this.len.to_inner_length(),
            }
        }
    }
}

// --- hoard impls ---

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeInnerBlobError<Peak: std::error::Error, Next: std::error::Error, Length: std::error::Error> {
    Peak(Peak),
    Next(Next),
    Length(Length),
}

impl<T, S: Copy, Z, P: PtrBlob, L: ToInnerLength> Blob for Inner<T, S, Z, P, L>
where T: Blob,
      S: Blob,
      Z: Blob,
      L: Blob,
{
    const SIZE: usize = <SumPerfectTree<T, S, Z, P, DummyHeight> as Blob>::SIZE +
                        <SumMMR<T, S, Z, P, DummyNonZeroLength> as Blob>::SIZE +
                        L::SIZE;

    type DecodeBytesError = DecodeInnerBlobError<!, !, !>;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> { todo!() }
}

unsafe impl<T, S: Copy, Z, P: PtrBlob> BlobDyn for InnerDyn<T, S, Z, P>
where T: Blob,
      S: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeInnerBlobError<!, !, !>;

    fn try_size(_: <Self as Pointee>::Metadata) -> std::result::Result<usize, <Self as Pointee>::LayoutError> { todo!() }

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeSumMMRBytesError<
    Z: std::error::Error,
    P: std::error::Error,
    S: std::error::Error,
    L: std::error::Error,
>{
    Zone(Z),
    TipPtr(P),
    Sum(S),
    Len(L),
}

impl<T, S: Copy, Z, P: PtrBlob, L: ToLength> Blob for SumMMR<T, S, Z, P, L>
where T: Blob,
      S: Blob,
      Z: Blob,
      L: Blob,
{
    const SIZE: usize = Z::SIZE + P::SIZE + <Digest as Blob>::SIZE + S::SIZE + L::SIZE;
    type DecodeBytesError = DecodeSumMMRBytesError<Z::DecodeBytesError, P::DecodeBytesError, S::DecodeBytesError, L::DecodeBytesError>;

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> { todo!() }
}

unsafe impl<T, S: Copy, Z, P: PtrBlob> BlobDyn for SumMMRDyn<T, S, Z, P>
where T: Blob,
      S: Blob,
      Z: Blob,
{
    type DecodeBytesError = DecodeSumMMRBytesError<Z::DecodeBytesError, P::DecodeBytesError, S::DecodeBytesError, !>;

    fn try_size(_: <Self as Pointee>::Metadata) -> std::result::Result<usize, <Self as Pointee>::LayoutError> { todo!() }

    fn encode_bytes<'a>(&self, _: BytesUninit<'a, Self>) -> Bytes<'a, Self> { todo!() }

    fn decode_bytes(_: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<<Self as IntoOwned>::Owned>, <Self as BlobDyn>::DecodeBytesError> { todo!() }
}

impl<T, S: Copy, Z: Zone, P: Ptr, L: ToLength> Load for SumMMR<T, S, Z, P, L>
where T: Load,
      S: Blob,
      L: Blob,
{
    type Blob = SumMMR<T::Blob, S, (), P::Blob, L>;
    type Zone = Z;

    fn load(_blob: Self::Blob, _zone: &<Self as Load>::Zone) -> Self {
        todo!()
    }
}

impl<T, S: Copy, Z: Zone, P: Ptr> LoadRef for SumMMRDyn<T, S, Z, P>
where T: Load,
      S: Blob,
{
    type BlobDyn = SumMMRDyn<T::Blob, S, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(_: hoard::blob::Bytes<'a, <Self as LoadRef>::BlobDyn>, _: &<Self as LoadRef>::Zone) -> std::result::Result<MaybeValid<hoard::owned::Ref<'a, Self>>, <<Self as LoadRef>::BlobDyn as BlobDyn>::DecodeBytesError> { todo!() }
}

impl<T, S: Copy, Z: Zone, P: Ptr, L: ToInnerLength> Load for Inner<T, S, Z, P, L>
where T: Load,
      S: Blob,
      L: Blob,
{
    type Blob = Inner<T::Blob, S, (), P::Blob, L>;
    type Zone = Z;

    fn load(_blob: Self::Blob, _zone: &<Self as Load>::Zone) -> Self {
        todo!()
    }
}

impl<T, S: Copy, Z: Zone, P: Ptr> LoadRef for InnerDyn<T, S, Z, P>
where T: Load,
      S: Blob,
{
    type BlobDyn = InnerDyn<T::Blob, S, (), P::Blob>;
    type Zone = Z;

    fn load_ref_from_bytes<'a>(_: hoard::blob::Bytes<'a, <Self as LoadRef>::BlobDyn>, _: &<Self as LoadRef>::Zone) -> std::result::Result<MaybeValid<hoard::owned::Ref<'a, Self>>, <<Self as LoadRef>::BlobDyn as BlobDyn>::DecodeBytesError> { todo!() }
}
