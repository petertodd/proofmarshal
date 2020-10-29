use std::mem::{self, ManuallyDrop};
use std::borrow::{Borrow, BorrowMut};
use std::lazy::SyncOnceCell;
use std::convert::TryFrom;
use std::ptr;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::fmt;

use hoard::pointee::Pointee;
use hoard::zone::{Alloc, Get, GetMut, Ptr, Zone};
use hoard::owned::{Take, Own, IntoOwned};
use hoard::bag::Bag;

use crate::collections::{
    length::*,
    perfecttree::{
        PerfectTree,
        raw::PerfectTreeRaw,
    },
};
use crate::unreachable_unchecked;

pub mod raw;
use self::raw::*;
pub use self::raw::Kind;

pub struct InnerNode<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: InnerNodeRaw<T, Z, P>,
    len: InnerLength,
}

pub struct InnerNodeDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: InnerNodeRaw<T, Z, P>,
    len: InnerLengthDyn,
}

pub struct InnerTip<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: InnerTipRaw<T, Z, P>,
    len: InnerLength,
}

pub struct InnerTipDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: InnerTipRaw<T, Z, P>,
    len: InnerLengthDyn,
}

pub struct PeakTree<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: PeakTreeRaw<T, Z, P>,
    len: NonZeroLength,
}

pub struct PeakTreeDyn<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    marker: PhantomData<T>,
    raw: PeakTreeRaw<T, Z, P>,
    len: NonZeroLengthDyn,
}

impl<T, Z, P: Ptr> PeakTree<T, Z, P> {
    pub fn from_peak(peak: PerfectTree<T, Z, P>) -> Self {
        let (peak, height) = peak.into_raw_parts();
        let raw = PeakTreeRaw::from_peak(peak);
        let len = NonZeroLength::from_height(height);
        unsafe {
            Self::from_raw_parts(raw, len)
        }
    }

    pub fn from_tip(tip: InnerTip<T, Z, P>) -> Self {
        let (tip, len) = tip.into_raw_parts();
        let raw = PeakTreeRaw::from_tip(tip);
        let len = NonZeroLength::from(len);
        unsafe {
            Self::from_raw_parts(raw, len)
        }
    }

    pub unsafe fn from_raw_parts(
        raw: PeakTreeRaw<T, Z, P>,
        len: NonZeroLength,
    ) -> Self
    {
        Self {
            marker: PhantomData,
            raw,
            len,
        }
    }
}

impl<T, Z, P: Ptr> PeakTreeDyn<T, Z, P> {
    pub fn len(&self) -> NonZeroLength {
        self.len.to_nonzero_length()
    }
}

impl<T, Z, P: Ptr> InnerTip<T, Z, P> {
    pub fn into_raw_parts(self) -> (InnerTipRaw<T, Z, P>, InnerLength) {
        let this = ManuallyDrop::new(self);

        unsafe {
            (ptr::read(&this.raw),
             ptr::read(&this.len))
        }
    }
}

impl<T, Z, P: Ptr> InnerTipDyn<T, Z, P> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }
}

impl<T, Z, P: Ptr> InnerNodeDyn<T, Z, P> {
    pub fn len(&self) -> InnerLength {
        self.len.to_inner_length()
    }

    pub fn left(&self) -> &PeakTreeDyn<T, Z, P> {
        unsafe { self.raw.left(self.len()) }
    }

    pub fn left_mut(&mut self) -> &mut PeakTreeDyn<T, Z, P> {
        unsafe { self.raw.left_mut(self.len()) }
    }

    pub fn right(&self) -> &PeakTreeDyn<T, Z, P> {
        unsafe { self.raw.right(self.len()) }
    }

    pub fn right_mut(&mut self) -> &mut PeakTreeDyn<T, Z, P> {
        unsafe { self.raw.right_mut(self.len()) }
    }
}

use hoard::zone::heap::Heap;
pub fn test_left(node: &InnerNodeDyn<u8, Heap>) -> &PeakTreeDyn<u8, Heap> {
    node.left()
}

pub fn test_right(node: &InnerNodeDyn<u8, Heap>) -> &PeakTreeDyn<u8, Heap> {
    node.right()
}

pub fn test_both(node: &InnerNodeDyn<u8, Heap>) -> (&PeakTreeDyn<u8, Heap>, &PeakTreeDyn<u8, Heap>) {
    (node.left(), node.right())
}

// ------- pointee stuff ----------
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
                let len: usize = len.into();
                let ptr = ptr::slice_from_raw_parts(thin, len.into());
                unsafe { mem::transmute(ptr) }
            }

            fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
                let len = len.get();
                let len: usize = len.into();
                let ptr = ptr::slice_from_raw_parts_mut(thin, len.into());
                unsafe { mem::transmute(ptr) }
            }
        }
    }
}

impl_pointee!(PeakTreeDyn, NonZeroLength);
impl_pointee!(InnerTipDyn, InnerLength);
impl_pointee!(InnerNodeDyn, InnerLength);

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
impl_deref!(InnerTip => InnerTipDyn);
impl_deref!(InnerNode => InnerNodeDyn);

// ---- drop impls -------

macro_rules! impl_drop {
    ($($t:ident,)+) => {$(
        impl<T, Z, P: Ptr> Drop for $t<T, Z, P> {
            fn drop(&mut self) {
                unsafe { self.raw.dealloc(self.len()) }
            }
        }
    )+}
}

impl_drop! {
    InnerNode, InnerNodeDyn,
    InnerTip, InnerTipDyn,
    PeakTree, PeakTreeDyn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
