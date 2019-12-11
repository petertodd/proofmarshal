use super::*;

use core::marker::PhantomData;
use core::mem::ManuallyDrop;

#[allow(unreachable_code)]
#[derive(Debug)]
pub struct NeverAllocator<P> {
    marker: PhantomData<fn(P) -> P>,
    never: !,
}

impl<P: Ptr> Alloc for NeverAllocator<P> {
    type Ptr = P;

    fn alloc<T: ?Sized + Pointee>(&mut self, _src: impl Take<T>) -> OwnedPtr<T, Self::Ptr> {
        match self.never {}
    }

    fn zone(&self) -> P::Zone {
        match self.never {}
    }
}

impl Ptr for ! {
    type Persist = !;
    type Zone = !;
    type Allocator = NeverAllocator<!>;

    fn allocator() -> Self::Allocator {
        unreachable!()
    }

    fn dealloc_owned<T: ?Sized + Pointee>(ptr: OwnedPtr<T,Self>) {
        match ptr.raw {}
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(ptr: OwnedPtr<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
        match ptr.raw {}
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist> {
        match ptr.raw {}
    }
}

impl PtrMut for ! {}

impl<P: Ptr> Get<P> for ! {
    fn get<'a, T: ?Sized + Pointee + Owned>(&self, _: &'a ValidPtr<T, P>) -> Ref<'a, T> {
        match *self {}
    }

    fn take<T: ?Sized + Pointee + Owned>(&self, _: OwnedPtr<T, P>) -> T::Owned {
        match *self {}
    }
}

impl<P: Ptr> GetMut<P> for ! {
    fn get_mut<'a, T: ?Sized + Pointee>(&self, _: &'a mut ValidPtr<T, P>) -> &'a mut T {
        match *self {}
    }
}
