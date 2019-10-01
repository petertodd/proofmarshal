//! Owned data storage.
//!
//! Key concept: this is about data that's actually owned, and can be dereferenced and allocated.

use core::fmt;

use crate::pointee::Pointee;
use crate::refs::Ref;

mod ptr;
pub use self::ptr::Ptr;




pub trait Arena : Sized {
    type Ptr : fmt::Debug;
    type Error : fmt::Debug;

    unsafe fn dealloc<T: ?Sized + Pointee>(raw: Self::Ptr, metadata: T::Metadata);

    /// Try to get the value behind a pointer.
    fn try_get<'p, T: ?Sized + Load<Self>>(&self, ptr: &'p Ptr<T, Self>) -> Result<Ref<'p, T>, Self::Error>;

    /// Try to dereference the pointer directly.
    ///
    /// Included for `Debug` impls.
    fn try_debug_deref<'p, T: ?Sized + Pointee>(ptr: &'p Ptr<T,Self>) -> Option<&'p T> {
        None
    }
}

/// A *value* that can be loaded from a pointer in an arena into memory.
///
/// This allows the value type to determine how it should be loaded from the arena. In the case of
/// in-memory values, `Load<A>` isn't even used: a reference to the value is simply returned by the
/// arena.
///
/// For other types of arenas, the appropriate method is called by the arena.
pub trait Load<A: Arena> : Pointee {
    type Error : 'static + fmt::Debug;

    /*
    fn load_blob<'p>(arena: &A, offset: &'p Ptr<Self, A::Offset>) -> Result<Ref<'p, Self>, Self::Error>
        where A: blob::Arena;
    */
}

/*
use core::any::type_name;
use core::fmt;
use core::mem;

use crate::ptr::{Ptr, Dealloc};
use crate::refs::Ref;

pub mod blob;
use self::blob::AllocBlob;


pub mod own;
use self::own::Own;

pub mod heap;
use self::heap::Heap;
//pub mod obstack;
//use self::obstack::Obstack;

pub mod primitive;


pub trait Avail : Arena {
    fn deref_ptr<'p, T: ?Sized + Load<Self>>(&self, ptr: &'p Ptr<T, Self::Ptr>) -> Ref<'p, T> {
        self.try_deref_ptr(ptr).unwrap()
    }
}



pub trait Store<A: Arena> : Pointee {
    fn store_blob(owned: Self::Owned, allocator: &mut impl blob::AllocBlob<A>) -> (A::Offset, Self::Metadata)
        where A: blob::Arena;
}


/// An allocator that can allocate values in an arena.
pub trait Alloc<A: Arena> {
    //fn alloc_owned<T: ?Sized + Store<A>>(&mut self, value: T::Owned) -> Own<T,A> {
    //    unimplemented!()
    //}

    fn alloc<T: Store<A>>(&mut self, value: T) -> Own<T,A>;
}

pub trait Global : Arena {
    type Allocator : Alloc<Self>;

    fn allocator() -> Self::Allocator;
}


#[cfg(test)]
mod tests {
    use super::*;

    use crate::tuple::Item;

    #[derive(Debug)]
    pub struct Foo(u8, u8);

    impl<A: Arena> Load<A> for Foo {
        type Error = !;

        fn load_blob<'p>(arena: &A, offset: &'p Ptr<Self, A::Offset>) -> Result<Ref<'p, Self>, Self::Error>
            where A: blob::Arena
        {
            unimplemented!()
        }
    }

    impl<A: Arena> Store<A> for Foo {
        fn store_blob(owned: Self::Owned, allocator: &mut impl AllocBlob<A>) -> (A::Offset, Self::Metadata)
            where A: blob::Arena
        {
            let prims = Item(owned.0, Item(owned.1, ()));

            allocator.alloc_blob(&prims).into_raw()
        }
    }

    #[test]
    fn test() {
        let _owned: Own<_, Heap> = Own::new(Foo(8,32));
    }
}*/
