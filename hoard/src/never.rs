use super::*;

use core::marker::PhantomData;
use core::mem::ManuallyDrop;

#[allow(unreachable_code)]
#[derive(Debug)]
pub struct NeverAllocator<Z> {
    marker: PhantomData<fn(Z) -> Z>,
    never: !,
}

impl<Z: Zone> Alloc for NeverAllocator<Z> {
    type Zone = Z;
    type Ptr = Z::Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, _src: impl Take<T>) -> Own<T, Z::Ptr> {
        match self.never {}
    }

    fn zone(&self) -> Self::Zone {
        match self.never {}
    }
}

impl Ptr for ! {
    fn dealloc_own<T: ?Sized + Pointee>(own: Own<T,Self>) {
        match *own.ptr() {}
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(own: Own<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
        match *own.ptr() {}
    }
}

impl Zone for ! {
    type Ptr = !;
    //type PersistPtr = !;
    type Allocator = NeverAllocator<!>;

    fn allocator() -> Self::Allocator {
        panic!()
    }

}

/*
impl Get for ! {
    fn get<'p, T: ?Sized + Owned + Pointee>(&self, _ptr: &'p Own<T, !>) -> Ref<'p, T> {
        match *self {}
    }

    fn take<T: ?Sized + Owned + Pointee>(&self, _ptr: Own<T, !>) -> T::Owned {
        match *self {}
    }
}
*/
