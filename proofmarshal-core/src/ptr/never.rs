use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

impl Ptr for ! {
    type Error = !;
    type Allocator = NeverAlloc<!>;

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self> { match *self {} }
    unsafe fn dealloc<T>(self) { match self {} }
}

impl TryGet for ! {
    unsafe fn try_get<'p, T: Clone>(&'p self) -> Result<Cow<'p, T>, Self::Error> {
        match *self {}
    }
    unsafe fn try_take<'p, T>(self) -> Result<T, Self::Error> {
        match self {}
    }
}

impl TryGetMut for ! {
    unsafe fn try_get_mut<'p, T>(&'p mut self) -> Result<&'p mut T, Self::Error> {
        match *self {}
    }
}

impl Get for ! {
    unsafe fn get<'p, T: Clone>(&'p self) -> Cow<'p, T> {
        match *self {}
    }
    unsafe fn take<'p, T>(self) -> T {
        match self {}
    }
}

impl GetMut for ! {
    unsafe fn get_mut<'p, T>(&'p mut self) -> &'p mut T {
        match *self {}
    }
}

/// Allocator for pointers that can't be actually be allocated (such as `!`).
pub struct NeverAlloc<P> {
    marker: PhantomData<P>,
    /// Makes `NeverAlloc` uninhabited.
    pub never: !,
}

impl<P: Ptr> Alloc for NeverAlloc<P> {
    type Ptr = P;

    fn alloc<T>(&mut self, _value: T) -> Own<T,Self::Ptr> {
        match self.never {}
    }
}
