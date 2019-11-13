#![feature(never_type)]
#![feature(associated_type_bounds)]
#![feature(alloc_layout_extra)]
#![feature(slice_from_raw_parts)]

use core::any::type_name;
use core::task::Poll;
use core::fmt;

use pointee::Pointee;
use owned::{Owned, Ref, Take};

pub mod pointee;
use self::pointee::*;

pub mod marshal;
use self::marshal::*;

pub mod own;
use self::own::Own;

pub mod bag;

pub mod never;
pub mod heap;
pub mod pile;

pub mod linkedlist;


pub trait Zone : Sized {
    type Ptr : fmt::Debug;
    type PersistPtr : 'static + fmt::Debug + Copy + Load<!> + Load<Self>;

    type Allocator : Alloc<Zone = Self>;

    fn allocator() -> Self::Allocator
        where Self: Default;

    unsafe fn dealloc_own<T: ?Sized + Pointee>(ptr: Self::Ptr, metadata: T::Metadata);

    fn fmt_debug_own<T: ?Sized + Pointee>(ptr: &Own<T, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug
    {
        f.debug_struct(type_name::<Own<T, Self>>())
            .field("ptr", ptr.ptr())
            .field("metadata", &ptr.metadata())
            .finish()
    }

    fn drop_take<T: ?Sized + Pointee + Owned>(ptr: Own<T, Self>) -> Option<T::Owned>;
}

pub trait Alloc : Sized {
    type Zone : Zone;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Zone>;
    fn zone(&self) -> Self::Zone;
}

impl<A: Alloc> Alloc for &'_ mut A {
    type Zone = A::Zone;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Zone> {
        (**self).alloc(src)
    }

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }
}

pub trait TryGet : Zone {
    type Error;

    fn get<'p, T: ?Sized + Load<Self>>(&self, ptr: &'p Own<T, Self>) -> Result<Ref<'p, T>, Self::Error>;
}

pub trait Get : Zone {
    fn get<'p, T: ?Sized + Load<Self>>(&self, ptr: &'p Own<T, Self>) -> Ref<'p, T>;

    fn take<'p, T: ?Sized + Load<Self>>(&self, ptr: Own<T, Self>) -> T::Owned;
}
