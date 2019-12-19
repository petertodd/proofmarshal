//! Uninhabited zones and pointers.

use super::*;

use owned::Owned;

use core::marker::PhantomData;
use core::mem::ManuallyDrop;

/// An uninhabited pointer allocator.
///
/// Useful when a `Ptr` doesn't implement `Default`.
#[allow(unreachable_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        match ptr.raw {}
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

impl<P: Ptr> Zone<P> for ! {
    fn get<'a, T: ?Sized + Pointee>(&self, _: &'a ValidPtr<T, P>) -> Ref<'a, T, P> {
        match *self {}
    }

    fn take<T: ?Sized + Pointee + Owned>(&self, _: OwnedPtr<T, P>) -> Own<T::Owned, P> {
        match *self {}
    }
}

impl<P: Ptr> ZoneMut<P> for ! {
    fn get_mut<'a, T: ?Sized + Pointee>(&self, _: &'a mut ValidPtr<T, P>) -> RefMut<'a, T, P> {
        match *self {}
    }
}
