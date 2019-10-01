/// Generic pointers

use core::borrow::Borrow;
use core::fmt;

use crate::pointee::Pointee;

pub mod own;
use self::own::Own;

pub mod refs;
use self::refs::Ref;

pub mod blob;
pub use self::blob::*;

pub mod heap;
pub use self::heap::Heap;
pub mod stack;

mod never;
pub use self::never::*;
mod missing;
pub use self::missing::Missing;

pub mod marshal;
use self::marshal::Type;

pub mod persist;
use self::persist::Persist;

/// Owned pointer deallocation.
///
/// This is the most basic arena trait.
pub trait Arena : Sized {
    type Ptr : fmt::Debug;

    unsafe fn dealloc<T: ?Sized + Pointee>(raw: Self::Ptr, metadata: T::Metadata);

    unsafe fn debug_deref<T: ?Sized + Pointee>(raw: &Self::Ptr, metadata: T::Metadata) -> Option<&T> {
        let _ = (raw, metadata);
        None
    }

    /*
    /// If an `Arena` implements `Clone` it's expected to be able to clone owned pointers.
    fn clone_own<T: Type<Self>>(&self, own: &Own<T,Self>) -> Own<T,Self>
        where Self: Clone,
              T: Clone,
    {
        let _ = own;
        unimplemented!()
    }
    */
}

/// An arena whose values are available.
pub trait Locate : Arena {
    type Error : fmt::Debug;
    type Locator : TryGet<Self>;
}

/// An arena that can allocate new values.
pub trait Allocate : Locate {
    type Allocator : Alloc<Arena=Self>;
}

pub trait Global : Allocate {
    fn allocator() -> Self::Allocator;
}

pub trait TryGet<A: Locate = Self> {
    fn try_get<'p, T: ?Sized + Type<A>>(&self, own: &'p Own<T,A>) -> Result<&'p T, A::Error>;
    fn try_take<T: Type<A>>(&self, own: Own<T,A>) -> Result<T, A::Error>;
}

pub trait Get<A: Arena = Self> {
    fn get<'p, T: ?Sized + Type<A>>(&self, own: &'p Own<T,A>) -> &'p T;
    fn take<T: Type<A>>(&self, own: Own<T,A>) -> T;
}

/// An allocator for an arena.
///
/// Allocators are also required to implement `TryTake`, so that you can use the allocator to get
/// access to the allocated values.
pub trait Alloc {
    type Arena : Locate;

    fn locator(&self) -> &<Self::Arena as Locate>::Locator;

    fn alloc<T: Type<Self::Arena>>(&mut self, value: T) -> Own<T,Self::Arena>;

    fn try_get<'p, T: ?Sized + Type<Self::Arena>>(&self, own: &'p Own<T,Self::Arena>)
        -> Result<&'p T, <Self::Arena as Locate>::Error>
    {
        self.locator().try_get(own)
    }
}

impl<'a, A: Locate, L: TryGet<A>> TryGet<A> for &'a L {
    #[inline]
    fn try_get<'p, T: ?Sized + Type<A>>(&self, own: &'p Own<T,A>) -> Result<&'p T, A::Error> {
        (**self).try_get(own)
    }

    #[inline]
    fn try_take<T: Type<A>>(&self, own: Own<T,A>) -> Result<T, A::Error> {
        (**self).try_take(own)
    }
}
impl<'a, A: Locate, L: TryGet<A>> TryGet<A> for &'a mut L {
    #[inline]
    fn try_get<'p, T: ?Sized + Type<A>>(&self, own: &'p Own<T,A>) -> Result<&'p T, A::Error> {
        (**self).try_get(own)
    }

    #[inline]
    fn try_take<T: Type<A>>(&self, own: Own<T,A>) -> Result<T, A::Error> {
        (**self).try_take(own)
    }
}

impl<'a, A: Alloc> Alloc for &'a mut A {
    type Arena = A::Arena;

    #[inline]
    fn locator(&self) -> &<Self::Arena as Locate>::Locator {
        (**self).locator()
    }

    #[inline]
    fn alloc<T: Type<Self::Arena>>(&mut self, value: T) -> Own<T,Self::Arena> {
        (**self).alloc(value)
    }
}

impl<T: Type<A>, A: Arena> Type<A> for Own<T,A> {
    type Error = !;
    type RefOwned = Self;

    fn store_blob<'a>(&self, _arena: &mut impl AllocBlob<A>) -> Own<Self, A>
        where A: BlobArena
    {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let owned = Own::new_in(42u8, &mut &mut Heap);

        dbg!(owned);
    }
}
