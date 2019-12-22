//! Uninhabited zones and pointers.

use super::*;

use owned::Owned;

use core::marker::PhantomData;
use core::mem::ManuallyDrop;

/// An uninhabited allocator.
///
/// Useful when a `Zone` doesn't implement `Default`.
#[allow(unreachable_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NeverAllocator<Z> {
    marker: PhantomData<fn() -> Z>,
    never: !,
}

impl<Z: Zone> Alloc for NeverAllocator<Z> {
    type Zone = Z;

    fn alloc<T: ?Sized + Pointee>(&mut self, _src: impl Take<T>) -> OwnedPtr<T, Self::Zone> {
        match self.never {}
    }

    /*
    fn zone(&self) -> P::Zone {
        match self.never {}
    }
    */
}

impl Zone for ! {
    type Ptr = !;
    type Persist = Self;
    type Allocator = NeverAllocator<Self>;
    type Error = !;

    fn allocator() -> Self::Allocator {
        unreachable!("! doesn't implement Default")
    }

    fn duplicate(&self) -> Self {
        match *self {}
    }

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        match ptr.raw {}
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        match ptr.raw {}
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        _: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        match owned.raw {}
    }
}

impl PersistZone for ! {
    type PersistPtr = !;
}

/*
impl PtrMut for ! {}

impl<P: Ptr> Zone<P> for ! {
    fn get<'a, T: ?Sized + Pointee>(&self, _: &'a ValidPtr<T, P>) -> Ref<'a, T, P> {
        match *self {}
    }

    /*
    fn take<T: ?Sized + Pointee + Owned>(&self, _: OwnedPtr<T, P>) -> Own<T::Owned, P> {
        match *self {}
    }
    */
}

impl<P: Ptr> ZoneMut<P> for ! {
    /*
    fn get_mut<'a, T: ?Sized + Pointee>(&self, _: &'a mut ValidPtr<T, P>) -> RefMut<'a, T, P> {
        match *self {}
    }
    */
}
*/
