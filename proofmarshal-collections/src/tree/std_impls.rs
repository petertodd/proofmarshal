use super::*;

use std::borrow::{Borrow, BorrowMut, ToOwned};

impl<T, S, P: Ptr, Z> AsRef<SumTreeDyn<T, S, P, Z>> for SumTree<T, S, P, Z> {
    fn as_ref(&self) -> &SumTreeDyn<T, S, P, Z> {
        unsafe {
            let ptr = ptr::slice_from_raw_parts(self as *const _ as *const (), self.height.into());
            mem::transmute(ptr)
        }
    }
}

impl<T, S, P: Ptr, Z> AsMut<SumTreeDyn<T, S, P, Z>> for SumTree<T, S, P, Z> {
    fn as_mut(&mut self) -> &mut SumTreeDyn<T, S, P, Z> {
        unsafe {
            let ptr = ptr::slice_from_raw_parts(self as *const _ as *const (), self.height.into());
            mem::transmute(ptr)
        }
    }
}

impl<T, S, P: Ptr, Z> Borrow<SumTreeDyn<T, S, P, Z>> for SumTree<T, S, P, Z> {
    fn borrow(&self) -> &SumTreeDyn<T, S, P, Z> {
        self.as_ref()
    }
}

impl<T, S, P: Ptr, Z> BorrowMut<SumTreeDyn<T, S, P, Z>> for SumTree<T, S, P, Z> {
    fn borrow_mut(&mut self) -> &mut SumTreeDyn<T, S, P, Z> {
        self.as_mut()
    }
}

impl<T, S, P: Ptr, Z> ops::Deref for SumTree<T, S, P, Z> {
    type Target = SumTreeDyn<T, S, P, Z>;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T, S, P: Ptr, Z> ops::DerefMut for SumTree<T, S, P, Z> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T, S, P: Ptr> AsRef<InnerDyn<T, S, P>> for Inner<T, S, P> {
    fn as_ref(&self) -> &InnerDyn<T, S, P> {
        unsafe {
            let ptr = ptr::slice_from_raw_parts(self as *const _ as *const (), self.height.into());
            mem::transmute(ptr)
        }
    }
}

impl<T, S, P: Ptr> AsMut<InnerDyn<T, S, P>> for Inner<T, S, P> {
    fn as_mut(&mut self) -> &mut InnerDyn<T, S, P> {
        unsafe {
            let ptr = ptr::slice_from_raw_parts(self as *const _ as *const (), self.height.into());
            mem::transmute(ptr)
        }
    }
}

impl<T, S, P: Ptr> Borrow<InnerDyn<T, S, P>> for Inner<T, S, P> {
    fn borrow(&self) -> &InnerDyn<T, S, P> {
        self.as_ref()
    }
}

impl<T, S, P: Ptr> BorrowMut<InnerDyn<T, S, P>> for Inner<T, S, P> {
    fn borrow_mut(&mut self) -> &mut InnerDyn<T, S, P> {
        self.as_mut()
    }
}

impl<T, S, P: Ptr> ops::Deref for Inner<T, S, P> {
    type Target = InnerDyn<T, S, P>;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T, S, P: Ptr> ops::DerefMut for Inner<T, S, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T, S, P: Ptr, Z> ToOwned for SumTreeDyn<T, S, P, Z>
where T: Clone, S: Clone, P: Clone, Z: Clone
{
    type Owned = SumTree<T, S, P, Z>;

    fn to_owned(&self) -> Self::Owned {
        todo!()
    }
}

impl<T, S, P: Ptr> ToOwned for InnerDyn<T, S, P>
where T: Clone, S: Clone, P: Clone
{
    type Owned = Inner<T, S, P>;

    fn to_owned(&self) -> Self::Owned {
        Inner {
            left:  ManuallyDrop::new(self.left().to_owned().into_data()),
            right: ManuallyDrop::new(self.right().to_owned().into_data()),
            height: self.height(),
        }
    }
}

impl<T, S, P> fmt::Debug for SumTreeData<T, S, P>
where S: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SumTreeData")
            .field("flags", &self.load_flags(Ordering::Relaxed))
            .field("tip_digest", &self.try_tip_digest())
            .field("sum", &self.try_sum())
            .field("tip", &self.tip)
            .finish()
    }
}

impl<T, S, P: Ptr, Z> fmt::Debug for SumTree<T, S, P, Z>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug, Z: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SumTree")
            .field("flags", &self.data.load_flags(Ordering::Relaxed))
            .field("tip_digest", &self.data.try_tip_digest())
            .field("sum", &self.data.try_sum())
            .field("tip", &self.get_dirty_tip())
            .field("zone", &self.zone)
            .field("height", &self.height)
            .finish()
    }
}

impl<T, S, P: Ptr, Z> fmt::Debug for SumTreeDyn<T, S, P, Z>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug, Z: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SumTreeDyn")
            .field("flags", &self.data.load_flags(Ordering::Relaxed))
            .field("tip_digest", &self.data.try_tip_digest())
            .field("sum", &self.data.try_sum())
            .field("tip", &self.get_dirty_tip())
            .field("zone", &self.zone)
            .field("height", &&self.height)
            .finish()
    }
}

impl<T: fmt::Debug, S, P: Ptr> fmt::Debug for Inner<T, S, P>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Inner")
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &self.height)
            .finish()
    }
}

impl<T, S, P: Ptr> fmt::Debug for InnerDyn<T, S, P>
where T: fmt::Debug, S: fmt::Debug, P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InnerDyn")
            .field("left", &self.left())
            .field("right", &self.right())
            .field("height", &&self.height)
            .finish()
    }
}
