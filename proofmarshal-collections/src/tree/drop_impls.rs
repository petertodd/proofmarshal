use super::*;

use std::ptr;

impl<T, S, P: Ptr> SumTreeData<T, S, P> {
    unsafe fn drop_tip(&mut self, height: Height) {
        if let Ok(height) = NonZeroHeight::try_from(height) {
            self.tip.dealloc::<InnerDyn<T, S, P>>(height);
        } else {
            self.tip.dealloc::<T>(());
        }
    }
}

impl<T, S, P: Ptr, Z> Drop for SumTree<T, S, P, Z> {
    fn drop(&mut self) {
        unsafe {
            self.data.drop_tip(self.height());
        }
    }
}

impl<T, S, P: Ptr, Z> Drop for SumTreeDyn<T, S, P, Z> {
    fn drop(&mut self) {
        unsafe {
            self.data.drop_tip(self.height());
        }
    }
}

impl<T, S, P: Ptr> Drop for Inner<T, S, P> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}

impl<T, S, P: Ptr> Drop for InnerDyn<T, S, P> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.left_mut());
            ptr::drop_in_place(self.right_mut());
        }
    }
}
