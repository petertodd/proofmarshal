//! Volatile, in-memory, zone allocation.

use core::ptr::{NonNull, copy_nonoverlapping, drop_in_place};
use core::mem::ManuallyDrop;
use core::fmt;

use std::alloc::Layout;

use super::*;

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

impl Zone for Heap {
    type Ptr = HeapPtr;
    //type PersistPtr = !;

    type Allocator = Self;

    fn allocator() -> Self { Self }

}

/*
impl Get for Heap {
    fn get<'p, T: ?Sized + Pointee + Owned>(&self, ptr: &'p Own<T, Self::Ptr>) -> Ref<'p, T> {
        let r: &'p T = unsafe { ptr.ptr().get(ptr.metadata()) };
        Ref::Borrowed(r)
    }

    fn take<T: ?Sized + Pointee + Owned>(&self, own: Own<T, Self::Ptr>) -> T::Owned {
        let (ptr, metadata) = own.into_raw_parts();

        unsafe { ptr.take::<T>(metadata) }
    }
}
*/

impl Alloc for Heap {
    type Zone = Heap;
    type Ptr = HeapPtr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::new_unchecked(
                FatPtr {
                    raw: HeapPtr::alloc::<T>(src),
                    metadata
                })
        })
    }

    fn zone(&self) -> Heap {
        Heap
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct HeapPtr(NonNull<()>);

impl Ptr for HeapPtr {
    fn dealloc_own<T: ?Sized + Pointee>(owned: Own<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            unsafe {
                core::ptr::drop_in_place(value)
            }
        )
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: Own<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        let FatPtr { raw: Self(non_null), metadata } = owned.into_inner();

        unsafe {
            let r: &mut T = &mut *T::make_fat_ptr_mut(non_null.as_ptr(), metadata);
            let r: &mut ManuallyDrop<T> = &mut *(r as *mut _ as *mut _);

            f(r);

            let layout = Layout::for_value(r);
            if layout.size() > 0 {
                std::alloc::dealloc(r as *mut _ as *mut u8, layout);
            }
        }
    }
}

impl HeapPtr {
    #[inline]
    unsafe fn alloc<T: ?Sized + Pointee>(src: &ManuallyDrop<T>) -> Self {
        let layout = Layout::for_value(src);

        if layout.size() > 0 {
            let dst = NonNull::new(std::alloc::alloc(layout))
                              .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

            copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                layout.size());

            HeapPtr(dst.cast())
        } else {
            HeapPtr(NonNull::new_unchecked(layout.align() as *mut ()))
        }
    }

    #[inline]
    unsafe fn get<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> &T {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr(thin, metadata);

        &*fat
    }

    #[inline]
    unsafe fn take<T: ?Sized + Pointee + Owned>(self, metadata: T::Metadata) -> T::Owned {
        let this = ManuallyDrop::new(self);

        let r: &mut T = &mut *T::make_fat_ptr_mut(this.0.as_ptr(), metadata);
        let layout = Layout::for_value(r);

        let owned = T::to_owned(&mut *(r as *mut T as *mut ManuallyDrop<T>));

        if layout.size() > 0 {
            std::alloc::dealloc(r as *mut _ as *mut u8, layout);
        };

        owned
    }

    #[inline]
    unsafe fn get_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> &mut T {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr_mut(thin, metadata);

        &mut *fat
    }
}

impl fmt::Pointer for HeapPtr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator() {
        //let _: Own<[u8], HeapPtr> = Heap.alloc(vec![1,2,3]);
    }

    #[test]
    fn empty_alloc() {
        unsafe {
            let raw = HeapPtr::alloc(&ManuallyDrop::new(()));
            assert_eq!(raw.0, NonNull::dangling());
        }
    }
}
