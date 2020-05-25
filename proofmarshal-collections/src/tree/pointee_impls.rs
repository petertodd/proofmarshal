use super::*;

use std::ptr;

use hoard::pointee::Pointee;

unsafe impl<T, S, P: Ptr, Z> Pointee for SumTreeDyn<T, S, P, Z> {
    type Metadata = Height;
    type LayoutError = !;

    /*
    #[inline(always)]
    fn try_layout(_: Height) -> Result<Layout, !> {
        Ok(Layout::new::<SumTree<T, S, P, ()>>())
    }
    */

    #[inline(always)]
    fn metadata(this: &Self) -> Height {
        this.height()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), height: Height) -> *const Self {
        unsafe {
            mem::transmute(slice::from_raw_parts(thin, height.into()))
        }
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), height: Height) -> *mut Self {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(thin, height.into()))
        }
    }
}

unsafe impl<T, S, P: Ptr> Pointee for InnerDyn<T, S, P> {
    type Metadata = NonZeroHeight;
    type LayoutError = !;

    /*
    #[inline(always)]
    fn try_layout(_: Height) -> Result<Layout, !> {
        Ok(Layout::new::<SumTree<T, S, P, ()>>())
    }
    */

    #[inline(always)]
    fn metadata(this: &Self) -> NonZeroHeight {
        this.height()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), height: NonZeroHeight) -> *const Self {
        unsafe {
            mem::transmute(slice::from_raw_parts(thin, height.into()))
        }
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), height: NonZeroHeight) -> *mut Self {
        unsafe {
            mem::transmute(slice::from_raw_parts_mut(thin, height.into()))
        }
    }
}

unsafe impl<T, S, P: Ptr, Z> Take<SumTreeDyn<T, S, P, Z>> for SumTree<T, S, P, Z> {
    #[inline(always)]
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<SumTreeDyn<T, S, P, Z>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this: &mut SumTreeDyn<T, S, P, Z> = this.borrow_mut();
        let this: &mut ManuallyDrop<_> = unsafe { &mut *(this as *mut _ as *mut _)};
        f(this)
    }
}

unsafe impl<T, S, P: Ptr> Take<InnerDyn<T, S, P>> for Inner<T, S, P> {
    #[inline(always)]
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<InnerDyn<T, S, P>>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let this: &mut InnerDyn<T, S, P> = this.borrow_mut();
        let this: &mut ManuallyDrop<_> = unsafe { &mut *(this as *mut _ as *mut _)};
        f(this)
    }
}

unsafe impl<T, S, P: Ptr, Z> IntoOwned for SumTreeDyn<T, S, P, Z> {
    type Owned = SumTree<T, S, P, Z>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        SumTree {
            height: this.height(),
            data: ptr::read(&this.data),
            zone: ptr::read(&this.zone),
        }
    }
}

unsafe impl<T, S, P: Ptr> IntoOwned for InnerDyn<T, S, P> {
    type Owned = Inner<T, S, P>;

    #[inline(always)]
    unsafe fn into_owned_unchecked(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        Inner {
            height: this.height(),
            left: ptr::read(&this.left),
            right: ptr::read(&this.right),
        }
    }
}
