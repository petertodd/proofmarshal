//! Volatile, in-memory, zone allocation.

use core::ptr::{NonNull, copy_nonoverlapping, drop_in_place};
use core::mem::{self, ManuallyDrop};
use core::fmt;

use std::alloc::Layout;

use super::*;

use crate::marshal::{Persist, Primitive, blob::{WriteBlob, Blob, FullyValidBlob, BlobLayout}};
use crate::never::NeverAllocator;
use crate::coerce::{TryCastRef, TryCast};

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

impl Zone<HeapPtr> for Heap {
    fn get<'a, T: ?Sized + Pointee>(&self, ptr: &'a ValidPtr<T, HeapPtr>) -> Ref<'a, T, HeapPtr> {
        let r: &'a T = HeapPtr::try_get_dirty(ptr).unwrap();
        r.into()
    }

    fn take<T: ?Sized + Pointee + Owned>(&self, owned: OwnedPtr<T, HeapPtr>) -> Own<T::Owned, HeapPtr> {
        HeapPtr::take_impl(owned, |value|
            unsafe {
                T::to_owned(value)
            }
        ).into()
    }
}

impl ZoneMut<HeapPtr> for Heap {
    fn get_mut<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut ValidPtr<T, HeapPtr>) -> RefMut<'a, T, HeapPtr> {
        RefMut {
            this: unsafe { &mut *T::make_fat_ptr_mut(ptr.raw.0.as_ptr(), ptr.metadata) },
            zone: Heap,
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

impl Default for HeapPtr {
    fn default() -> Self {
        Self(NonNull::dangling())
    }
}

impl From<!> for HeapPtr {
    fn from(never: !) -> Self {
        match never {}
    }
}

impl Ptr for HeapPtr {
    type Persist = NeverPersist;
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

#[allow(unreachable_code)]
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct NeverPersist {
    ptr: NonNull<()>,
    never: !,
}

unsafe impl NonZero for NeverPersist {}
unsafe impl Persist for NeverPersist {}

impl From<NeverPersist> for HeapPtr {
    fn from(never: NeverPersist) -> HeapPtr {
        match never.never {}
    }
}

impl Primitive for NeverPersist {
    type Error = !;
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        match self.never {}
    }

    fn validate_blob<'a, Q: Ptr>(_: Blob<'a, Self, Q>) -> Result<FullyValidBlob<'a, Self, Q>, !> {
        panic!()
    }

    fn deref_blob<'a, Q: Ptr>(_: FullyValidBlob<'a, Self, Q>) -> &'a Self {
        panic!()
    }
}

impl Ptr for NeverPersist {
    type Persist = NeverPersist;
    type Zone = !;
    type Allocator = NeverAllocator<Self>;

    #[inline]
    fn allocator() -> Self::Allocator {
        unreachable!()
    }

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        match ptr.raw.never {}
    }

    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>) {
        match owned.raw.never {}
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        match owned.raw.never {}
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist> {
        match ptr.raw.never {}
    }
}

unsafe impl TryCastRef<HeapPtr> for NeverPersist {
    type Error = !;

    fn try_cast_ref(&self) -> Result<&HeapPtr, !> {
        match self.never {}
    }
}

unsafe impl TryCast<HeapPtr> for NeverPersist {
    fn try_cast(self) -> Result<HeapPtr, !> {
        match self.never {}
    }
}

unsafe impl TryCastRef<NeverPersist> for NeverPersist {
    type Error = !;

    fn try_cast_ref(&self) -> Result<&Self, !> {
        match self.never {}
    }
}

unsafe impl TryCast<NeverPersist> for NeverPersist {
    fn try_cast(self) -> Result<Self, !> {
        match self.never {}
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
