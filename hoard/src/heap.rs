//! Volatile, in-memory, zone allocation.

use core::ptr::{NonNull, copy_nonoverlapping, drop_in_place};
use core::mem::ManuallyDrop;
use core::fmt;

use std::alloc::Layout;

use super::*;

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

impl Get<HeapPtr> for Heap {
    fn get<'a, T: ?Sized + Pointee + Owned>(&self, ptr: &'a ValidPtr<T, HeapPtr>) -> Ref<'a, T> {
        let r: &'a T = HeapPtr::try_get_dirty(ptr).unwrap();
        Ref::Borrowed(r)
    }

    fn take<T: ?Sized + Pointee + Owned>(&self, owned: OwnedPtr<T, HeapPtr>) -> T::Owned {
        HeapPtr::take_impl(owned, |value|
            unsafe {
                T::to_owned(value)
            }
        )
    }
}

impl GetMut<HeapPtr> for Heap {
    fn get_mut<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut ValidPtr<T, HeapPtr>) -> &'a mut T {
        unsafe {
            &mut *T::make_fat_ptr_mut(ptr.raw.0.as_ptr(), ptr.metadata)
        }
    }
}

impl Alloc for Heap {
    type Ptr = HeapPtr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, HeapPtr> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            OwnedPtr::new_unchecked(
                ValidPtr::new_unchecked(
                    FatPtr {
                        raw: HeapPtr::alloc::<T>(src),
                        metadata
                    }
                )
            )
        })
    }

    #[inline]
    fn zone(&self) -> Heap { Heap }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct HeapPtr(NonNull<()>);

unsafe impl NonZero for HeapPtr {}

/// Safe because `HeapPtr` doesn't implement marshalling.
unsafe impl Persist for HeapPtr {}

impl From<!> for HeapPtr {
    fn from(never: !) -> Self {
        match never {}
    }
}

impl Ptr for HeapPtr {
    type Persist = !;
    type Zone = Heap;
    type Allocator = Heap;

    #[inline]
    fn allocator() -> Heap { Heap }

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        let cloned = Self::try_get_dirty(ptr).unwrap().clone();

        Heap.alloc(cloned)
    }

    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            unsafe {
                core::ptr::drop_in_place(value)
            }
        )
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        HeapPtr::take_impl(owned, f);
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist> {
        unsafe {
            Ok(&*T::make_fat_ptr(ptr.raw.0.as_ptr(), ptr.metadata))
        }
    }
}

impl PtrMut for HeapPtr {}

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

    fn take_impl<T, R>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>) -> R) -> R
        where T: ?Sized + Pointee
    {
        let FatPtr { raw: Self(non_null), metadata } = owned.into_inner().into();

        unsafe {
            let value: &mut T = &mut *T::make_fat_ptr_mut(non_null.as_ptr(), metadata);
            let value: &mut ManuallyDrop<T> = &mut *(value as *mut _ as *mut _);

            let r = f(value);

            let layout = Layout::for_value(value);
            if layout.size() > 0 {
                std::alloc::dealloc(value as *mut _ as *mut u8, layout);
            };

            r
        }
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
