/// In-memory arena
pub trait Arena {
    /// Default allocator.
    type Allocator : Alloc<Self>;

    fn dealloc<T: ?Sized + Pointee>(&mut self, ptr: Mem<Self>);
}

/// A memory allocator.
pub trait Alloc<A> {
    type Error;

    /// Safe because the worst that can happen is memory gets leaked.
    ///
    /// Lifetime of the memory tied to the lifetime of the arena.
    fn alloc(&mut self, layout: Layout) -> Result<Mem<A>, Self::Error>;
}

impl<A> Alloc<A> for ! {
    type Error = !;

    fn alloc(&mut self, _: Layout) -> Result<Mem<A>, Self::Error> {
        match *self {}
    }
}
