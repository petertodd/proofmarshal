use std::convert::TryFrom;
use std::fmt;
use std::lazy::SyncOnceCell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;

use thiserror::Error;

use hoard::blob::{Blob, Bytes, BytesUninit};
use hoard::pointee::Pointee;
use hoard::zone::{Ptr, PtrBlob};
use hoard::load::{Load, MaybeValid};

use crate::commit::Digest;

use super::{
    leaf::Leaf,
    super::height::*,
    PerfectTree, PerfectTreeDyn,
    InnerTip, InnerTipDyn,
    InnerNode, InnerNodeDyn,
};

pub struct InnerTipRaw<T, Z, P: Ptr> {
    marker: PhantomData<T>,
    pub digest: SyncOnceCell<Digest>,
    pub zone: Z,
    pub ptr: P,
}

pub struct InnerNodeRaw<T, Z, P: Ptr> {
    pub left: ManuallyDrop<PerfectTreeRaw<T, Z, P>>,
    pub right: ManuallyDrop<PerfectTreeRaw<T, Z, P>>,
}

pub union PerfectTreeRaw<T, Z, P: Ptr> {
    pub leaf: ManuallyDrop<Leaf<T, Z, P>>,
    pub tip: ManuallyDrop<InnerTipRaw<T, Z, P>>,
}

#[derive(Debug)]
pub enum Kind<Leaf, Tip> {
    Leaf(Leaf),
    Tip(Tip),
}

impl<T, Z, P: Ptr> InnerNodeRaw<T, Z, P> {
    pub fn new(left: PerfectTreeRaw<T, Z, P>, right: PerfectTreeRaw<T, Z, P>) -> Self {
        Self {
            left: ManuallyDrop::new(left),
            right: ManuallyDrop::new(right),
        }
    }

    pub unsafe fn left(&self, height: NonZeroHeight) -> &PerfectTreeDyn<T, Z, P> {
	&*PerfectTreeDyn::make_fat_ptr(&self.left as *const _ as *const _, height.decrement())
    }

    pub unsafe fn left_mut(&mut self, height: NonZeroHeight) -> &mut PerfectTreeDyn<T, Z, P> {
	&mut *PerfectTreeDyn::make_fat_ptr_mut(&mut self.left as *mut _ as *mut _, height.decrement())
    }

    pub unsafe fn right(&self, height: NonZeroHeight) -> &PerfectTreeDyn<T, Z, P> {
	&*PerfectTreeDyn::make_fat_ptr(&self.right as *const _ as *const _, height.decrement())
    }

    pub unsafe fn right_mut(&mut self, height: NonZeroHeight) -> &mut PerfectTreeDyn<T, Z, P> {
	&mut *PerfectTreeDyn::make_fat_ptr_mut(&mut self.right as *mut _ as *mut _, height.decrement())
    }

    pub unsafe fn dealloc(&mut self, height: NonZeroHeight) {
        ptr::drop_in_place(self.left_mut(height));
        ptr::drop_in_place(self.right_mut(height));
    }
}

impl<T, Z, P: Ptr> InnerTipRaw<T, Z, P> {
    pub fn from_raw_parts(digest: Option<Digest>, zone: Z, ptr: P) -> Self {
        Self {
            marker: PhantomData,
            digest: digest.map(SyncOnceCell::from).unwrap_or_else(SyncOnceCell::new),
            zone,
            ptr
        }
    }


    pub unsafe fn dealloc(&mut self, height: NonZeroHeight) {
        self.ptr.dealloc::<InnerNodeDyn<T, Z, P>>(height);
    }
}

impl<T, Z, P: Ptr> PerfectTreeRaw<T, Z, P> {
    pub fn from_leaf(leaf: Leaf<T, Z, P>) -> Self {
        Self {
            leaf: ManuallyDrop::new(leaf)
        }
    }

    pub fn from_tip(tip: InnerTipRaw<T, Z, P>) -> Self {
        Self {
            tip: ManuallyDrop::new(tip)
        }
    }

    pub unsafe fn kind(&self, height: Height) -> Kind<&Leaf<T, Z, P>, &InnerTipDyn<T, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(height) {
            Kind::Tip(
                &*InnerTipDyn::make_fat_ptr(&self.tip as *const _ as *const _, height)
            )
        } else {
            Kind::Leaf(&self.leaf)
        }
    }

    pub unsafe fn kind_mut(&mut self, height: Height) -> Kind<&mut Leaf<T, Z, P>, &mut InnerTipDyn<T, Z, P>> {
        if let Ok(height) = NonZeroHeight::try_from(height) {
            Kind::Tip(
                &mut *InnerTipDyn::make_fat_ptr_mut(&mut self.tip as *mut _ as *mut _, height)
            )
        } else {
            Kind::Leaf(&mut self.leaf)
        }
    }

    pub unsafe fn dealloc(&mut self, height: Height) {
        match self.kind_mut(height) {
            Kind::Tip(tip) => ptr::drop_in_place(tip),
            Kind::Leaf(leaf) => ptr::drop_in_place(leaf),
        }
    }
}
