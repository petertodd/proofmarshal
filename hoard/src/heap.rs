//! Volatile, in-memory, zone allocation.

use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::cmp;
use std::mem::ManuallyDrop;

use owned::{Take, IntoOwned};

use crate::pointee::Pointee;
use crate::ptr::*;
//use crate::bag::Bag;
use crate::refs::Ref;
use crate::load::*;
use crate::blob::*;

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct HeapPtr(pub(crate) NonNull<u16>);

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

impl Default for HeapPtr {
    fn default() -> Self {
        Self(NonNull::dangling())
    }
}

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

pub(crate) unsafe fn heap_alloc(layout: Layout) -> NonNull<u16> {
    if layout.size() > 0 {
        let layout = min_align_layout(layout);

        NonNull::new(std::alloc::alloc(layout))
                .unwrap_or_else(|| std::alloc::handle_alloc_error(layout))
                .cast()
    } else {
        NonNull::new_unchecked(layout.align() as *mut u16)
    }
}

pub(crate) unsafe fn heap_dealloc(ptr: NonNull<u16>, layout: Layout) {
    if layout.size() > 0 {
        std::alloc::dealloc(ptr.as_ptr().cast(), min_align_layout(layout))
    };
}

pub(crate) unsafe fn alloc_unchecked_impl<T: ?Sized>(src: &mut ManuallyDrop<T>) -> NonNull<u16> {
    let layout = Layout::for_value(src);
    let dst = heap_alloc(layout);

    std::ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr().cast(),
                                  layout.size());
    dst
}

pub(crate) unsafe fn dealloc_impl<T: ?Sized + Pointee>(ptr: NonNull<u16>, metadata: T::Metadata) {
    let value = &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
    let layout = Layout::for_value(value);

    std::ptr::drop_in_place(value);
    heap_dealloc(ptr, layout)
}

impl Ptr for HeapPtr {
    type Persist = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        dealloc_impl::<T>(self.0, metadata)
    }

    fn alloc<T: ?Sized + Pointee, U: Take<T>>(src: U) -> Own<T, Self> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            let layout = Layout::for_value(src);
            let dst = heap_alloc(layout);

            std::ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr().cast(),
                                          layout.size());

            Own::new_unchecked(Fat::new(HeapPtr(dst), metadata))
        })
    }

    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_unchecked_with<T, U, F>(&self, metadata: T::Metadata, f: F) -> Own<T, Self>
        where T: ?Sized + Pointee,
              F: FnOnce(&T) -> U,
              U: Take<T>,
    {
        let owned = self.try_get_dirty_unchecked::<T>(metadata)
                        .map(f).into_ok();

        Self::alloc(owned)
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        Ok(&*T::make_fat_ptr(self.0.cast().as_ptr(), metadata))
    }

    unsafe fn try_take_dirty_unchecked<T: ?Sized + Pointee>(self, metadata: T::Metadata) -> Result<T::Owned, Self::Persist>
        where T: IntoOwned
    {
        let value = &mut *T::make_fat_ptr_mut(self.0.cast().as_ptr(), metadata);
        let layout = Layout::for_value(value);

        let owned = T::into_owned_unchecked(&mut *(value as *mut _ as *mut ManuallyDrop<T>));
        heap_dealloc(self.0, layout);
        Ok(owned)
    }
}

impl ValidateBlob for HeapPtr {
    const BLOB_LEN: usize = 0;
    type Error = !;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.finish()) }
    }
}

impl Decode<HeapPtr> for HeapPtr {
    fn decode_blob(blob: BlobDecoder<HeapPtr, Self>) -> Self {
        panic!()
    }
}

impl ValidateBlob for Heap {
    const BLOB_LEN: usize = 0;
    type Error = !;

    fn validate_blob<'a>(blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.finish()) }
    }
}

impl Decode<HeapPtr> for Heap {
    fn decode_blob(blob: BlobDecoder<HeapPtr, Self>) -> Self {
        unreachable!("HeapPtr is not a BlobPtr")
    }
}

impl Get<HeapPtr> for Heap {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a HeapPtr, metadata: T::Metadata) -> Ref<'a, T>
        where T: IntoOwned
    {
        ptr.try_get_dirty_unchecked::<T>(metadata)
           .into_ok().into()
    }

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: HeapPtr, metadata: T::Metadata) -> T::Owned
        where T: IntoOwned
    {
        ptr.try_take_dirty_unchecked::<T>(metadata)
           .into_ok()
    }
}

impl GetMut<HeapPtr> for Heap {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut HeapPtr, metadata: T::Metadata) -> &'a mut T {
        &mut *T::make_fat_ptr_mut(ptr.0.cast().as_ptr(), metadata)
    }
}

impl Alloc for Heap {
    type Zone = Self;
    type Ptr = HeapPtr;

    fn zone(&self) -> Self::Zone {
        Heap
    }

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr> {
        HeapPtr::alloc(src)
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get() {
        let bag = Bag::new_in(123, Heap);

        let r: Ref<u8> = bag.get();
        assert_eq!(*r, 123u8);

        let bag = Bag::new_in(123u8, Heap);
    }

    #[test]
    fn get_mut() {
        let mut bag = Bag::new_in(1u8, Heap);

        let r = bag.get_mut();
        *r += 1;

        let r = bag.get();
        assert_eq!(*r, 2);
    }

    /*
    #[test]
    fn clone() {
        let bag = Bag::new_in(42u8, Heap);
        let bag2 = bag.clone();
    }
    */
}
*/
