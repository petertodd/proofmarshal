#![feature(associated_type_bounds)]
#![feature(alloc_layout_extra)]
#![feature(slice_from_raw_parts)]
#![feature(manually_drop_take)]
#![feature(arbitrary_self_types)]
#![feature(const_if_match)]

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("64-bit pointers required");

#[cfg(not(target_endian = "little"))]
compile_error!("little endian required");

use core::any::type_name;
use core::fmt;
use core::mem::ManuallyDrop;
use core::task::Poll;

use pointee::Pointee;
use owned::{Owned, Ref, Take};

pub mod coerce;

pub mod pointee;
use self::pointee::*;

pub mod marshal;
use self::marshal::{Encode, Decode, Load, Primitive};

mod fatptr;
pub use self::fatptr::FatPtr;

mod validptr;
pub use self::validptr::ValidPtr;

mod ownedptr;
pub use self::ownedptr::OwnedPtr;

pub mod never;
pub mod heap;
pub mod pile;

pub mod hoard;

//pub mod bag;

//pub mod linkedlist;

/// Generic pointer.
pub trait Ptr : Sized + fmt::Debug {
    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>);

    fn fmt_debug_own<T: ?Sized + Pointee>(owned: &OwnedPtr<T, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug
    {
        f.debug_struct(type_name::<OwnedPtr<T, Self>>())
            .field("raw", &owned.raw)
            .field("metadata", &owned.metadata)
            .finish()
    }

    fn drop_take<T>(owned: OwnedPtr<T, Self>) -> Option<T> {
        let mut r = None;

        Self::drop_take_unsized(owned,
            |src| unsafe {
                r = Some(ManuallyDrop::take(src));
            }
        );

        r
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>));
}
pub trait Zone : Sized {
    type Ptr : Ptr;
    type PersistPtr : Ptr + Primitive + Into<Self::Ptr>;

    type Allocator : Alloc<Zone = Self>;

    fn allocator() -> Self::Allocator
        where Self: Default;
}

pub trait Alloc : Sized {
    type Zone : Zone;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, <Self::Zone as Zone>::Ptr>;
    fn zone(&self) -> Self::Zone;
}

impl<A: Alloc> Alloc for &'_ mut A {
    type Zone = A::Zone;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, <Self::Zone as Zone>::Ptr> {
        (**self).alloc(src)
    }

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }
}

pub trait TryGet : Zone {
    type Error;

    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a OwnedPtr<T, Self::Ptr>) -> Result<Ref<'a, T>, Self::Error>;
}

pub trait Get : Zone {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a OwnedPtr<T, Self::Ptr>) -> Ref<'a, T>
        where Self: 'a;
    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self::Ptr>) -> T::Owned;
}
