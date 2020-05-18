//! Volatile, in-memory, zone allocation.

use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::cmp;

use owned::{Take, IntoOwned};

use crate::pointee::Pointee;
use crate::ptr::*;
use crate::zone::*;
use crate::bag::Bag;
use crate::refs::Ref;

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct HeapPtr(NonNull<u16>);

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl Ptr for HeapPtr {
    type Persist = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        let value = &mut *T::make_fat_ptr_mut(self.0.cast().as_ptr(), metadata);
        let layout = Layout::for_value(value);

        std::ptr::drop_in_place(value);
        std::alloc::dealloc(self.0.cast().as_ptr(), layout);
    }

    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        todo!()
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        Ok(&*T::make_fat_ptr(self.0.cast().as_ptr(), metadata))
    }
}

impl Zone for Heap {
    type Ptr = HeapPtr;

    /*
    unsafe fn clone_ptr_unchecked<T: Clone>(ptr: &Self::Ptr) -> Self::Ptr {
        todo!()
    }

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            let layout = min_align_layout(Layout::for_value(src));

            let ptr = if layout.size() > 0 {
                let dst = NonNull::new(std::alloc::alloc(layout))
                    .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

                std::ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                              layout.size());

                dst.cast()
            } else {
                NonNull::new_unchecked(layout.align() as *mut u16)
            };

            Bag::from_owned_ptr(
                OwnedPtr::new_unchecked(
                    FatPtr::new(HeapPtr(ptr), metadata)
                ),
                Heap,
            )
        })
    }
    */
}

/*
impl Get for Heap {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a Self::Ptr, metadata: T::Metadata) -> Ref<'a, T>
        where T: IntoOwned
    {
        let r: &'a T = &*T::make_fat_ptr(ptr.0.cast().as_ptr(), metadata);
        Ref::Ref(r)
    }

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: Self::Ptr, metadata: T::Metadata) -> T::Owned
        where T: IntoOwned
    {
        todo!()
    }
}

impl GetMut for Heap {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut Self::Ptr, metadata: T::Metadata) -> &'a mut T {
        &mut *T::make_fat_ptr_mut(ptr.0.cast().as_ptr(), metadata)
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get() {
        let bag = Heap::alloc(123u8);

        let r: Ref<u8> = bag.get();
        assert_eq!(*r, 123u8);

        let bag = Heap::alloc(123u8);
    }

    #[test]
    fn get_mut() {
        let mut bag = Heap::alloc(1u8);

        let r = bag.get_mut();
        *r += 1;

        let r = bag.get();
        assert_eq!(*r, 2);
    }
}
*/
*/
