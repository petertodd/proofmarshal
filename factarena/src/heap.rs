use core::ptr;
use core::mem;

use std::alloc as rust_alloc;

use super::*;

#[derive(Debug)]
pub struct HeapPtr(ptr::NonNull<()>);

impl HeapPtr {
    unsafe fn into_box<T: ?Sized + Pointee>(self, metadata: T::Metadata) -> Box<T> {
        let thin = self.0.as_ptr();
        mem::forget(self);

        let fat = T::make_fat_ptr_mut(thin, metadata);
        Box::from_raw(fat)
    }

    unsafe fn as_inner<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> &T {
        let p: *const T = T::make_fat_ptr(self.0.as_ptr(), metadata);
        &*p
    }
}

impl Drop for HeapPtr {
    fn drop(&mut self) {
        panic!("HeapPtr dropped; shouldn't happen");
    }
}

impl Ptr for HeapPtr {
    unsafe fn get<'p, T: ?Sized + Load<Self>>(&'p self, metadata: T::Metadata) -> Ref<'p, T> {
        Ref::Borrowed(self.as_inner::<T>(metadata))
    }

    unsafe fn take<T: Load<Self>>(self, metadata: T::Metadata) -> T {
        let boxed_t = self.into_box::<T>(metadata);
        *boxed_t
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(self, metadata: T::Metadata) {
        self.into_box::<T>(metadata);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Heap;

impl Alloc<HeapPtr> for Heap {
    #[inline(always)]
    unsafe fn alloc_raw<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata, f: impl FnOnce(*mut T)) -> HeapPtr {
        let layout = T::layout(metadata);

        let thin = NonNull::new(rust_alloc::alloc(layout))
                           .unwrap_or_else(|| rust_alloc::handle_alloc_error(layout));

        let fat = T::make_fat_non_null(thin.cast(), metadata);

        f(fat.as_ptr());

        HeapPtr(fat.cast())
    }
}
