use std::ptr::{self, NonNull};
use std::alloc::{self, Layout};
use std::mem::ManuallyDrop;

use crate::load::MaybeValid;
use crate::owned::Own;

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct Heap;

#[derive(Debug)]
pub struct HeapPtr(NonNull<()>);

impl Ptr for HeapPtr {
    type Clean = !;
    type Blob = !;

    fn from_clean(never: !) -> Self {
        match never {}
    }


    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, !> {
        let ptr = T::make_fat_ptr_mut(self.0.as_ptr(), metadata);
        Ok(&*ptr)
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<&mut T, !> {
        let ptr = T::make_fat_ptr_mut(self.0.as_ptr(), metadata);
        Ok(&mut *ptr)
    }

    unsafe fn try_take_dirty_with<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> R
        where F: FnOnce(Result<Own<T>, Self::Clean>) -> R
    {
        let ptr = T::make_fat_ptr_mut(self.0.as_ptr(), metadata);

        let layout = Layout::for_value(&*ptr);

        let src: &mut ManuallyDrop<T> = &mut *(ptr as *mut _);
        let src: Own<T> = Own::new_unchecked(src);
        let r = f(Ok(src));

        if layout.size() > 0 {
            alloc::dealloc(ptr.cast(), layout);
        };

        r
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        let this = Self(self.0);
        this.try_take_dirty_with::<T, _, ()>(metadata, |_| ());
    }

    fn alloc_raw_impl(layout: Layout) -> (NonNull<()>, Self) {
        let ptr = if layout.size() > 0 {
            let ptr = unsafe { alloc::alloc(layout) };
            NonNull::new(ptr as *mut ())
                    .unwrap_or_else(|| alloc::handle_alloc_error(layout))
        } else {
            NonNull::new(layout.align() as *mut ()).unwrap()
        };

        (ptr, HeapPtr(ptr))
    }
}

impl AsPtr<Self> for HeapPtr {
    fn as_ptr(&self) -> &Self {
        self
    }
}

impl FromPtr<Self> for HeapPtr {
    fn from_ptr(this: Self) -> Self {
        this
    }
}

impl From<!> for HeapPtr {
    fn from(never: !) -> Self {
        match never {}
    }
}

impl Default for HeapPtr {
    fn default() -> Self {
        HeapPtr(NonNull::dangling())
    }
}

impl Get for Heap {
    unsafe fn get_unchecked<'p, T: ?Sized + LoadRef>(&self, ptr: &'p HeapPtr, metadata: T::Metadata)
        -> Result<MaybeValid<Ref<'p, T>>, Self::Error>
    {
        let r = ptr.try_get_dirty::<T>(metadata).into_ok();
        Ok(Ref::Borrowed(r).into())
    }

    unsafe fn take_unchecked<'p, T: ?Sized + LoadRef>(&self, ptr: HeapPtr, metadata: T::Metadata)
        -> Result<MaybeValid<T::Owned>, Self::Error>
    {
        todo!()
    }
}

impl GetMut for Heap {
    unsafe fn get_unchecked_mut<'p, T: ?Sized + Pointee>(&self, ptr: &'p mut HeapPtr, metadata: T::Metadata)
        -> Result<MaybeValid<&'p mut T>, Self::Error>
    {
        let r = ptr.try_get_dirty_mut::<T>(metadata).into_ok();
        Ok(r.into())
    }
}

impl AsZone<Heap> for Heap {
    fn as_zone(&self) -> &Self {
        self
    }
}

impl Zone for Heap {
    type Error = !;
    type Ptr = HeapPtr;
}

impl Alloc for Heap {
    fn alloc_raw(&mut self, layout: core::alloc::Layout) -> (NonNull<()>, Self::Ptr, Self) {
        let (nonnull, ptr) = HeapPtr::alloc_raw(layout);
        (nonnull, ptr, Heap)
    }
}
