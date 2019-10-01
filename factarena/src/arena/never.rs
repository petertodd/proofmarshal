use core::marker::PhantomData;

use super::*;

/// An allocator for arenas that don't allocate.
#[derive(Debug)]
#[allow(unreachable_code)]
pub struct NeverAlloc<A> {
    marker: PhantomData<A>,

    never: !,
}

impl<A> NeverAlloc<A> {
    /// "Create" a new `NeverAlloc` allocator.
    ///
    /// This will immediately panic.
    pub fn unimplemented() -> NeverAlloc<A> {
        unimplemented!()
    }
}

impl Arena for ! {
    type Ptr = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: !, _: T::Metadata) {
        match ptr {}
    }
}

impl Locate for ! {
    type Error = !;
    type Locator = !;
}

impl Allocate for ! {
    type Allocator = NeverAlloc<Self>;
}

impl<A: Locate> TryGet<A> for ! {
    fn try_get<'p, T: ?Sized + Type<A>>(&self, _: &'p Own<T,A>) -> Result<&'p T, A::Error> {
        match *self {}
    }
    fn try_take<T: Type<A>>(&self, _: Own<T,A>) -> Result<T, A::Error> {
        match *self {}
    }
}

impl<A: Locate> Alloc for NeverAlloc<A> {
    type Arena = A;

    fn locator(&self) -> &<Self::Arena as Locate>::Locator {
        match self.never {}
    }

    fn alloc<T: Type<A>>(&mut self, _: T) -> Own<T,A> {
        match self.never {}
    }
}

impl<A: Locate> TryGet<A> for NeverAlloc<A> {
    fn try_get<'p, T: ?Sized + Type<A>>(&self, _: &'p Own<T,A>) -> Result<&'p T, A::Error> {
        match self.never {}
    }
    fn try_take<T: Type<A>>(&self, _: Own<T,A>) -> Result<T, A::Error> {
        match self.never {}
    }
}
