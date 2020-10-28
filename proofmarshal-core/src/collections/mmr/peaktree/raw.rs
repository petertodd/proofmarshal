use std::lazy::SyncOnceCell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;

use hoard::zone::Ptr;
use hoard::pointee::Pointee;

use crate::commit::Digest;
use crate::collections::perfecttree::{
    raw::PerfectTreeRaw,
    PerfectTreeDyn,
};

use super::{
    PeakTreeDyn,
    InnerTipDyn,
    InnerNodeDyn,
    InnerLength,
    NonZeroLength,
};

pub struct InnerNodeRaw<T, Z, P: Ptr> {
    pub left: ManuallyDrop<PeakTreeRaw<T, Z, P>>,
    pub right: ManuallyDrop<PeakTreeRaw<T, Z, P>>,
}

pub struct InnerTipRaw<T, Z, P: Ptr> {
    marker: PhantomData<T>,
    digest: SyncOnceCell<Digest>,
    zone: Z,
    ptr: P,
}

pub union PeakTreeRaw<T, Z, P: Ptr> {
    peak: ManuallyDrop<PerfectTreeRaw<T, Z, P>>,
    tip: ManuallyDrop<InnerTipRaw<T, Z, P>>,
}

#[derive(Debug)]
pub enum Kind<Peak, Tip> {
    Peak(Peak),
    Tip(Tip),
}

impl<T, Z, P: Ptr> PeakTreeRaw<T, Z, P> {
    pub fn from_peak(peak: PerfectTreeRaw<T, Z, P>) -> Self {
        Self {
            peak: ManuallyDrop::new(peak),
        }
    }

    pub fn from_tip(tip: InnerTipRaw<T, Z, P>) -> Self {
        Self {
            tip: ManuallyDrop::new(tip)
        }
    }

    pub unsafe fn kind(&self, len: NonZeroLength) -> Kind<&PerfectTreeDyn<T, Z, P>, &InnerTipDyn<T, Z, P>> {
        match len.try_into_inner_length() {
            Ok(len) => {
                Kind::Tip(
                    &*InnerTipDyn::make_fat_ptr(&self.tip as *const _ as *const _, len)
                )
            },
            Err(height) => {
                Kind::Peak(
                    &*PerfectTreeDyn::make_fat_ptr(&self.peak as *const _ as *const _, height)
                )
            },
        }
    }

    pub unsafe fn kind_mut(&mut self, len: NonZeroLength) -> Kind<&mut PerfectTreeDyn<T, Z, P>, &mut InnerTipDyn<T, Z, P>> {
        match len.try_into_inner_length() {
            Ok(len) => {
                Kind::Tip(
                    &mut *InnerTipDyn::make_fat_ptr_mut(&mut self.tip as *mut _ as *mut _, len)
                )
            },
            Err(height) => {
                Kind::Peak(
                    &mut *PerfectTreeDyn::make_fat_ptr_mut(&mut self.peak as *mut _ as *mut _, height)
                )
            },
        }
    }

    pub unsafe fn dealloc(&mut self, len: NonZeroLength) {
        match self.kind_mut(len) {
            Kind::Tip(tip) => ptr::drop_in_place(tip),
            Kind::Peak(peak) => ptr::drop_in_place(peak),
        }
    }
}

impl<T, Z, P: Ptr> InnerTipRaw<T, Z, P> {
    pub unsafe fn dealloc(&mut self, len: InnerLength) {
        self.ptr.dealloc::<InnerNodeDyn<T, Z, P>>(len);
    }
}

impl<T, Z, P: Ptr> InnerNodeRaw<T, Z, P> {
    pub unsafe fn left(&self, len: InnerLength) -> &PeakTreeDyn<T, Z, P> {
        let (left, _right) = len.split();
        &*PeakTreeDyn::make_fat_ptr(&self.left as *const _ as *const _, left)
    }

    pub unsafe fn left_mut(&mut self, len: InnerLength) -> &mut PeakTreeDyn<T, Z, P> {
        let (left, _right) = len.split();
        &mut *PeakTreeDyn::make_fat_ptr_mut(&mut self.left as *mut _ as *mut _, left)
    }

    pub unsafe fn right(&self, len: InnerLength) -> &PeakTreeDyn<T, Z, P> {
        let (right, _right) = len.split();
        &*PeakTreeDyn::make_fat_ptr(&self.right as *const _ as *const _, right)
    }

    pub unsafe fn right_mut(&mut self, len: InnerLength) -> &mut PeakTreeDyn<T, Z, P> {
        let (right, _right) = len.split();
        &mut *PeakTreeDyn::make_fat_ptr_mut(&mut self.right as *mut _ as *mut _, right)
    }

    pub unsafe fn dealloc(&mut self, len: InnerLength) {
        ptr::drop_in_place(self.left_mut(len));
        ptr::drop_in_place(self.right_mut(len));
    }
}
