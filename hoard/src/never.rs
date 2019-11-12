use super::*;

use core::marker::PhantomData;

#[allow(unreachable_code)]
#[derive(Debug)]
pub struct NeverAllocator<Z> {
    marker: PhantomData<fn(Z) -> Z>,
    never: !,
}

impl<Z: Zone> Alloc for NeverAllocator<Z> {
    type Zone = Z;

    fn alloc<T: ?Sized + Pointee>(&mut self, _src: impl Take<T>) -> Own<T, Self::Zone> {
        match self.never {}
    }

    fn zone(&self) -> Self::Zone {
        match self.never {}
    }
}

impl Zone for ! {
    type Ptr = !;
    type PersistPtr = !;
    type Allocator = NeverAllocator<!>;

    unsafe fn dealloc_own<T: ?Sized + Pointee>(ptr: Self::Ptr, _: T::Metadata) {
        match ptr {}
    }

    fn allocator() -> Self::Allocator {
        panic!()
    }
}

impl Get for ! {
    fn get<'p, T: ?Sized + Owned + Pointee>(&self, _ptr: &'p Own<T, Self>) -> Ref<'p, T> {
        match *self {}
    }

    fn take<T: ?Sized + Owned + Pointee>(&self, _ptr: Own<T, Self>) -> T::Owned {
        match *self {}
    }
}
