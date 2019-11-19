#![feature(never_type)]
#![feature(associated_type_bounds)]
#![feature(alloc_layout_extra)]
#![feature(slice_from_raw_parts)]
#![feature(manually_drop_take)]

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use core::any::type_name;
use core::fmt;
use core::mem::ManuallyDrop;
use core::task::Poll;

use pointee::Pointee;
use owned::{Owned, Ref, Take};

pub mod pointee;
use self::pointee::*;

pub mod marshal;
use self::marshal::Load;

mod fatptr;
pub use self::fatptr::FatPtr;

mod own;
pub use self::own::Own;

pub mod never;
pub mod heap;
//pub mod pile;

pub mod hoard;

pub mod bag;

//pub mod linkedlist;

/// Generic pointer.
pub trait Ptr : Sized + fmt::Debug {
    fn dealloc_own<T: ?Sized + Pointee>(owned: Own<T, Self>);

    fn fmt_debug_own<T: ?Sized + Pointee>(owned: &Own<T, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug
    {
        f.debug_struct(type_name::<Own<T, Self>>())
            .field("raw", &owned.raw)
            .field("metadata", &owned.metadata)
            .finish()
    }

    fn drop_take<T>(owned: Own<T, Self>) -> Option<T> {
        let mut r = None;

        Self::drop_take_unsized(owned,
            |src| unsafe {
                r = Some(ManuallyDrop::take(src));
            }
        );

        r
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: Own<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>));
}
pub trait Zone : Sized {
    type Ptr : Ptr;

    type Allocator : Alloc<Zone = Self, Ptr = Self::Ptr>;

    fn allocator() -> Self::Allocator
        where Self: Default;
}

pub trait Alloc : Sized {
    type Zone : Zone<Ptr=Self::Ptr>;
    type Ptr : Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr>;
    fn zone(&self) -> Self::Zone;
}

impl<A: Alloc> Alloc for &'_ mut A {
    type Zone = A::Zone;
    type Ptr = A::Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        (**self).alloc(src)
    }

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }
}

pub trait TryGet : Zone {
    type Error;

    fn get<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: &'p Own<T, Self::Ptr>) -> Result<Ref<'p, T>, Self::Error>;
}

pub trait Get : Zone {
    fn get<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: &'p Own<T, Self::Ptr>) -> Ref<'p, T>;
    fn take<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned;
}
