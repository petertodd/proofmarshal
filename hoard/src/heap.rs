//! Volatile, in-memory, zone allocation.

use core::ptr::{NonNull, copy_nonoverlapping, drop_in_place};
use core::mem::{self, ManuallyDrop};
use core::fmt;

use std::alloc::Layout;

use nonzero::NonZero;
use owned::Take;

use crate::{
    pointee::Pointee,
    zone::{
        Alloc,
        OwnedPtr, ValidPtr, FatPtr,
        Zone,
    },
};

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct HeapPtr(NonNull<()>);

unsafe impl Send for HeapPtr {}
unsafe impl Sync for HeapPtr {}

impl Zone for Heap {
    type Ptr = HeapPtr;
    type Persist = !;
    type PersistPtr = !;

    type Allocator = Self;
    type Error = !;

    #[inline(always)]
    fn allocator() -> Self { Heap }

    #[inline(always)]
    fn duplicate(&self) -> Self { Heap }

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        let cloned = Self::try_get_dirty(ptr).unwrap().clone();
        Heap.alloc(cloned)
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        todo!()
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        todo!()
    }
}

/*
impl ZoneMut<HeapPtr> for Heap {
    fn get_mut<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut ValidPtr<T, HeapPtr>) -> RefMut<'a, T, HeapPtr> {
        RefMut {
            this: unsafe { &mut *T::make_fat_ptr_mut(ptr.raw.0.as_ptr(), ptr.metadata) },
            zone: Heap,
        }
    }
}
*/

impl Alloc for Heap {
    type Zone = Heap;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, Heap> {
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
}


unsafe impl NonZero for HeapPtr {}

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

    fn take_impl<T, R>(owned: OwnedPtr<T, Heap>, f: impl FnOnce(&mut ManuallyDrop<T>) -> R) -> R
        where T: ?Sized + Pointee
    {
        let owned = ManuallyDrop::new(owned);
        let FatPtr { raw: Self(non_null), metadata } = FatPtr::clone(&owned);

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
