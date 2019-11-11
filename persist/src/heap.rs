//! Volatile, in-memory, zone allocation.

use core::ptr::{NonNull, copy_nonoverlapping, drop_in_place};
use core::mem::ManuallyDrop;
use core::fmt;

use std::alloc::Layout;

use super::*;

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

impl Zone for Heap {
    type Ptr = Ptr;
    type PersistPtr = !;

    type Allocator = Self;

    fn allocator() -> Self { Self }

    unsafe fn dealloc_own<T: ?Sized + Pointee>(ptr: Self::Ptr, metadata: T::Metadata) {
        ptr.dealloc::<T>(metadata)
    }
}

impl Get for Heap {
    fn get<'p, T: ?Sized + Pointee + Owned>(&self, ptr: &'p Own<T, Self>) -> Ref<'p, T> {
        let r: &'p T = unsafe { ptr.ptr().get(ptr.metadata()) };
        Ref::Borrowed(r)
    }

    fn take<T: ?Sized + Load<Self>>(&self, own: Own<T, Self>) -> T::Owned {
        let (ptr, metadata) = own.into_raw_parts();

        unsafe { ptr.take::<T>(metadata) }
    }
}

impl Alloc for Heap {
    type Zone = Heap;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Zone> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::from_raw_parts(Ptr::alloc::<T>(src),
                                metadata)
        })
    }

    fn zone(&self) -> Heap {
        Heap
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Ptr(NonNull<()>);


impl Ptr {
    #[inline]
    unsafe fn alloc<T: ?Sized + Pointee>(src: &ManuallyDrop<T>) -> Self {
        let layout = Layout::for_value(src);

        if layout.size() > 0 {
            let dst = NonNull::new(std::alloc::alloc(layout))
                              .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

            copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                layout.size());

            Ptr(dst.cast())
        } else {
            Ptr(NonNull::new_unchecked(layout.align() as *mut ()))
        }
    }

    #[inline]
    unsafe fn dealloc<T: ?Sized + Pointee>(self, metadata: T::Metadata) {
        let r: &mut T = &mut *T::make_fat_ptr_mut(self.0.as_ptr(), metadata);
        let layout = Layout::for_value(r);

        drop_in_place(r);

        if layout.size() > 0 {
            std::alloc::dealloc(r as *mut _ as *mut u8, layout);
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

impl fmt::Pointer for Ptr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator() {
        let _: Own<[u8], Heap> = Heap.alloc(vec![1,2,3]);
    }

    #[test]
    fn empty_alloc() {
        unsafe {
            let raw = Ptr::alloc(&ManuallyDrop::new(()));
            assert_eq!(raw.0, NonNull::dangling());

            raw.dealloc::<()>(());
        }
    }
}

/*
    #[inline]
    unsafe fn into_box<T: ?Sized + Pointee>(self, metadata: T::Metadata) -> Box<T> {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr_mut(thin, metadata);
        Box::from_raw(fat)
    }

}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Allocator;

impl Zone for Heap {
    type Ptr = Raw;
    type Allocator = Allocator;
    type Error = !;

    #[inline]
    fn allocator() -> Self::Allocator { Allocator }

    fn clone_rec<T: Clone>(r: &Rec<T,Self>) -> Rec<T,Self> {
        /*
        let orig = unsafe { ptr.raw.get::<T>(()) };
        let cloned = orig.clone();
        let ptr = Ptr::from_box(Box::new(cloned));

        unsafe { Unique::from_raw_parts(ptr, ()) }
        */
        unimplemented!()
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: super::Ptr<T,Self>) {
        ptr.raw.into_box::<T>(ptr.metadata);
    }

    fn fmt_debug_rec<T: ?Sized + Pointee>(rec: &Rec<T,Self>, f: &mut fmt::Formatter) -> fmt::Result
        where T: fmt::Debug
    {
        let value = unsafe { rec.ptr.raw.get::<T>(rec.ptr.metadata) };
        fmt::Debug::fmt(value, f)
    }
}

impl Alloc for Allocator {
    type Zone = Heap;

    #[inline]
    fn alloc<T>(&mut self, value: T) -> Rec<T,Self::Zone> {
        let raw = Raw::from_box(Box::new(self));
        unsafe { Rec::from_ptr(super::Ptr { raw, metadata: () }) }
    }

    #[inline]
    fn zone(&self) -> Self::Zone { Heap }
}

impl TryGet for Heap {
    fn try_get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Result<Ref<'p, T, Self>, !> {
        let r: &T = unsafe { r.ptr().raw.get::<T>(r.ptr().metadata) };
        Ok(Ref::Borrowed(r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Foo(u8,u16);

    #[test]
    fn test() {
        //let bag = Bag::<_,Heap>::new(Foo(8,16));
        //dbg!(bag.get());
    }
}
*/
