use core::marker::PhantomData;

use super::*;

/// Dummy allocator for zones that can't actually allocate.
///
/// Implements `Alloc` for `Z`, but being an uninhabited type can't actually be created.
#[allow(unreachable_code)]
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct NeverAlloc<Z> {
    _marker: PhantomData<fn(Z)>,
    never: !,
}

impl<Z: Zone> Alloc for NeverAlloc<Z> {
    type Zone = Z;

    fn alloc<T>(&mut self, _value: T) -> Rec<T,Self::Zone> {
        match self.never {}
    }

    fn zone(&self) -> Self::Zone {
        match self.never {}
    }
}

impl Zone for ! {
    type Ptr = !;
    type Allocator = NeverAlloc<!>;
    type Error = !;

    fn clone_rec<T: Clone>(r: &Rec<T,Self>) -> Rec<T,Self> {
        match r.ptr().raw {}
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T,Self>) {
        match ptr.raw {}
    }
}
