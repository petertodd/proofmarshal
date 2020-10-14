use std::marker::PhantomData;
use std::borrow::{Borrow, BorrowMut};
use std::cell::Cell;
use std::mem::{self, ManuallyDrop};
use std::ops::DerefMut;
use std::convert::TryFrom;
use std::ptr;

use hoard::zone::{Alloc, Zone, Ptr};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, Own};
use hoard::bag::Bag;

pub mod height;
use self::height::*;

#[derive(Debug)]
pub struct SumPerfectTree<T, S: Copy, Z = (), P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToHeight = Height> {
    marker: PhantomData<T>,
    tip_digest: Cell<Option<[u8; 32]>>,
    sum: Cell<Option<S>>,
    ptr: P,
    zone: Z,
    height: H,
}

pub type SumPerfectTreeDyn<T, S, Z, P> = SumPerfectTree<T, S, Z, P, DynHeight>;

pub type PerfectTree<T, Z, P> = SumPerfectTree<T, (), Z, P>;
pub type PerfectTreeDyn<T, Z, P> = SumPerfectTreeDyn<T, (), Z, P>;

#[derive(Debug)]
pub struct Inner<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr, H: ?Sized + ToNonZeroHeight = NonZeroHeight> {
    left:  ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    right: ManuallyDrop<SumPerfectTree<T, S, Z, P, DummyHeight>>,
    height: H,
}

pub type InnerDyn<T, S, Z, P = <Z as Zone>::Ptr> = Inner<T, S, Z, P, DynNonZeroHeight>;

#[derive(Debug)]
pub enum TipRef<'a, T, S: Copy, Z, P: Ptr> {
    Leaf(&'a T),
    Inner(&'a InnerDyn<T, S, Z, P>),
}

#[derive(Debug)]
pub enum TipMut<'a, T, S: Copy, Z, P: Ptr> {
    Leaf(&'a mut T),
    Inner(&'a mut InnerDyn<T, S, Z, P>),
}

#[derive(Debug)]
pub enum JoinError<T, S: Copy, Z, P: Ptr = <Z as Zone>::Ptr> {
    HeightMismatch {
        lhs: SumPerfectTree<T, S, Z, P>,
        rhs: SumPerfectTree<T, S, Z, P>,
    },
    HeightOverflow {
        lhs: SumPerfectTree<T, S, Z, P>,
        rhs: SumPerfectTree<T, S, Z, P>,
    },
    SumOverflow {
        lhs: SumPerfectTree<T, S, Z, P>,
        rhs: SumPerfectTree<T, S, Z, P>,
    },
}

impl<T, S: Copy, Z: Zone> SumPerfectTree<T, S, Z> {
    pub fn new_leaf_in(value: T, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        let (ptr, (), zone) = zone.borrow_mut().alloc(value).into_raw_parts();

        Self {
            marker: PhantomData,
            tip_digest: None.into(),
            sum: None.into(),
            zone,
            ptr,
            height: Height::new(0).unwrap(),
        }
    }

    pub fn try_join(self, rhs: Self) -> Result<Self, JoinError<T, S, Z>>
        where Z: Alloc
    {
        let mut zone = self.zone;
        Inner::try_join(self, rhs, zone).map(|inner| {
            let inner_bag: Bag<InnerDyn<T, S, Z>, Z> = zone.alloc(inner);
            let (ptr, nonzero_height, _) = inner_bag.into_raw_parts();

            Self {
                marker: PhantomData,
                tip_digest: None.into(),
                sum: None.into(),
                ptr,
                zone,
                height: nonzero_height.to_height(),
            }
        })
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ToHeight> SumPerfectTree<T, S, Z, P, H> {
    fn strip(self) -> SumPerfectTree<T, S, Z, P, DummyHeight> {
        let this = ManuallyDrop::new(self);
        unsafe {
            SumPerfectTree {
                marker: PhantomData,
                tip_digest: ptr::read(&this.tip_digest),
                sum: ptr::read(&this.sum),
                ptr: ptr::read(&this.ptr),
                zone: ptr::read(&this.zone),
                height: DummyHeight,
             }
        }
    }
}


impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> SumPerfectTree<T, S, Z, P, H> {
    pub fn height(&self) -> Height {
        self.height.to_height()
    }

    pub fn try_get_dirty_tip<'a>(&'a self) -> Result<TipRef<'a, T, S, Z, P>, P::Clean> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe { self.ptr.try_get_dirty(height)? };
            Ok(TipRef::Inner(inner))
        } else {
            let leaf = unsafe { self.ptr.try_get_dirty(())? };
            Ok(TipRef::Leaf(leaf))
        }
    }

    pub fn try_get_dirty_tip_mut<'a>(&'a mut self) -> Result<TipMut<'a, T, S, Z, P>, P::Clean> {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            let inner = unsafe { self.ptr.try_get_dirty_mut(height)? };
            Ok(TipMut::Inner(inner))
        } else {
            let leaf = unsafe { self.ptr.try_get_dirty_mut(())? };
            Ok(TipMut::Leaf(leaf))
        }
    }
}

impl<T, S: Copy, Z: Zone> Inner<T, S, Z> {
    pub fn try_join(lhs: SumPerfectTree<T, S, Z>, rhs: SumPerfectTree<T, S, Z>, zone: Z) -> Result<Self, JoinError<T, S, Z>> {
        if lhs.height() != rhs.height() {
            Err(JoinError::HeightMismatch { lhs, rhs })
        } else if let Some(height) = lhs.height().try_increment() {
            unsafe {
                Ok(Self::new_unchecked(lhs, rhs, height))
            }
        } else {
            Err(JoinError::HeightOverflow { lhs, rhs })
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> Inner<T, S, Z, P> {
    unsafe fn new_unchecked<HL: ToHeight, HR: ToHeight>(
        left: SumPerfectTree<T, S, Z, P, HL>,
        right: SumPerfectTree<T, S, Z, P, HR>,
        height: NonZeroHeight,
    ) -> Self {
        Self {
            left: ManuallyDrop::new(left.strip()),
            right: ManuallyDrop::new(right.strip()),
            height,
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Inner<T, S, Z, P, H> {
    pub fn left(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(&self.left as *const _ as *const _, self.height().decrement())
        }
    }

    pub fn left_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(&mut self.left as *mut _ as *mut _, self.height().decrement())
        }
    }

    pub fn right(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(&self.right as *const _ as *const _, self.height().decrement())
        }
    }

    pub fn right_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(&mut self.right as *mut _ as *mut _, self.height().decrement())
        }
    }

    pub fn height(&self) -> NonZeroHeight {
        self.height.to_nonzero_height()
    }
}

// ---- drop impls -----
impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToHeight> Drop for SumPerfectTree<T, S, Z, P, H> {
    fn drop(&mut self) {
        if let Ok(height) = NonZeroHeight::try_from(self.height()) {
            unsafe { self.ptr.dealloc::<InnerDyn<T, S, Z, P>>(height) }
        } else {
            unsafe { self.ptr.dealloc::<T>(()) }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr, H: ?Sized + ToNonZeroHeight> Drop for Inner<T, S, Z, P, H> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}


// ------ unsized type stuff ------

impl<T, S: Copy, Z, P: Ptr> Pointee for SumPerfectTreeDyn<T, S, Z, P> {
    type Metadata = Height;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            Height::try_from(len)
                   .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), height: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, height.get().into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, height.get().into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S: Copy, Z, P: Ptr> Borrow<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn borrow(&self) -> &SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &*SumPerfectTreeDyn::make_fat_ptr(self as *const _ as *const (), self.height)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> BorrowMut<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut SumPerfectTreeDyn<T, S, Z, P> {
        unsafe {
            &mut *SumPerfectTreeDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
        }
    }
}

unsafe impl<T, S: Copy, Z, P: Ptr> Take<SumPerfectTreeDyn<T, S, Z, P>> for SumPerfectTree<T, S, Z, P> {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(Own<SumPerfectTreeDyn<T, S, Z, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this_dyn: &mut SumPerfectTreeDyn<T, S, Z, P> = this.deref_mut().borrow_mut();

        unsafe {
            f(Own::new_unchecked(this_dyn))
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> IntoOwned for SumPerfectTreeDyn<T, S, Z, P> {
    type Owned = SumPerfectTree<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            SumPerfectTree {
                height: this.height.to_height(),
                marker: PhantomData,
                tip_digest: ptr::read(&this.tip_digest),
                sum: ptr::read(&this.sum),
                zone: ptr::read(&this.zone),
                ptr: ptr::read(&this.ptr),
            }
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> Pointee for InnerDyn<T, S, Z, P> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    fn metadata(ptr: *const Self) -> Self::Metadata {
        unsafe {
            let ptr: *const [()] = mem::transmute(ptr);
            let len: usize = ptr.len();

            NonZeroHeight::try_from(len)
                          .expect("valid metadata")
        }
    }

    fn make_fat_ptr(thin: *const (), height: Self::Metadata) -> *const Self {
        let ptr = ptr::slice_from_raw_parts(thin, height.to_height().get().into());
        unsafe { mem::transmute(ptr) }
    }

    fn make_fat_ptr_mut(thin: *mut (), height: Self::Metadata) -> *mut Self {
        let ptr = ptr::slice_from_raw_parts_mut(thin, height.to_height().get().into());
        unsafe { mem::transmute(ptr) }
    }
}

impl<T, S: Copy, Z, P: Ptr> Borrow<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow(&self) -> &InnerDyn<T, S, Z, P> {
        unsafe {
            &*InnerDyn::make_fat_ptr(self as *const _ as *const (), self.height)
        }
    }
}

impl<T, S: Copy, Z, P: Ptr> BorrowMut<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
    fn borrow_mut(&mut self) -> &mut InnerDyn<T, S, Z, P> {
        unsafe {
            &mut *InnerDyn::make_fat_ptr_mut(self as *mut _ as *mut (), self.height)
        }
    }
}

unsafe impl<T, S: Copy, Z, P: Ptr> Take<InnerDyn<T, S, Z, P>> for Inner<T, S, Z, P> {
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

impl<T, S: Copy, Z, P: Ptr> IntoOwned for InnerDyn<T, S, Z, P> {
    type Owned = Inner<T, S, Z, P>;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = Own::leak(self);

        unsafe {
            Inner {
                height: this.height.to_nonzero_height(),
                left: ptr::read(&this.left),
                right: ptr::read(&this.right),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test() {
        let ll = PerfectTree::new_leaf_in(1u8, Heap);
        assert_eq!(ll.height().get(), 0);

        let lr = PerfectTree::new_leaf_in(2u8, Heap);
        assert_eq!(lr.height().get(), 0);

        let tip = ll.try_join(lr).unwrap();
        dbg!(tip.try_get_dirty_tip());
    }
}
