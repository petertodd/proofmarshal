//! # Hoard
//!
//! Hoard is a framework for persistently storing arbitrary data-structures to disk with
//! copy-on-write semantics. Hoard achives this by generalizing the notion of a pointer: rather
//! than solely pointing to volatile memory, pointers can be, for example, be an offset within a
//! memory-mapped database file, or a hash digest obtained remotely. This is combined with highly
//! efficient and robust serialization and deserialization, based on simple fixed-size formats that
//! match in-memory representations sufficiently closely to allow data to be directly memory-mapped
//! from disk.
//!
//! This means that like the Serde framework, "hoarded" datatypes can be accessed in the same way
//! as any other Rust data: structs and enums are simply structs and enums. Unlike Serde, Hoard's
//! support for pointers means you can load data on demand: a non-volatile tree stored in a file
//! can be accessed in almost exactly the same way as a volatile tree stored on the heap.
//!
//! Mutation is via copy-on-write: mutating data behind mutable pointers transparently makes a
//! mutable copy on the heap. When you're ready to save the data, the changes are written to disk
//! in an atomic transaction; unmodified data is left unchanged.

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

use nonzero::NonZero;
use pointee::Pointee;
use owned::{Owned, Take};

pub mod coerce;

pub mod pointee;
use self::pointee::*;

pub mod marshal;
use self::marshal::{Encode, Decode, Load, Primitive, Persist};

pub mod refs;
use self::refs::{Ref, RefMut, Own};

mod fatptr;
pub use self::fatptr::FatPtr;

mod validptr;
pub use self::validptr::ValidPtr;

mod ownedptr;
pub use self::ownedptr::OwnedPtr;

pub mod never;
pub mod heap;
//pub mod pile;

//pub mod hoard;

//pub mod bag;

//pub mod linkedlist;

/// Generic pointer.
pub trait Ptr : Sized + NonZero + Persist + fmt::Debug {
    /// The persistent version of this pointer, if applicable.
    ///
    /// # Safety
    ///
    /// If this is an inhabited type, it must have the same layout as `Self`. Ideally this would be
    /// expressed as a `Cast<Self>` bound on `Persist`. But this is awkward to implement as
    /// `Persist` has a `Copy` bound that `Self` does not.
    type Persist : Ptr + Primitive + coerce::Cast<Self> + Into<Self> + Copy;

    type Zone : Zone<Self> + Copy + Eq + Ord + core::hash::Hash + fmt::Debug;
    type Allocator : Alloc<Ptr=Self> + Eq + Ord + core::hash::Hash + fmt::Debug;

    fn allocator() -> Self::Allocator where Self: Default;

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self>
        where Self: Clone;

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

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist>;
}

pub trait PtrMut : Ptr<Zone : ZoneMut<Self>> {
}

pub trait Zone<P: Ptr> {
    fn get<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T, P>) -> Ref<'a, T, P>;

    fn take<T: ?Sized + Load<P>>(&self, ptr: OwnedPtr<T, P>) -> Own<T::Owned, P>;

    /*
    fn get_ref<'a, T: ?Sized + Load<P>>(&self, ptr: impl Into<Ref<'a, OwnedPtr<T, P>>>) -> Ref<'a, T>
        where P: 'a
    {
        match ptr.into() {
            Ref::Borrowed(ptr) => self.get(ptr),
            Ref::Owned(ptr) => Ref::Owned(self.take(ptr)),
        }
    }
    */
}

pub trait ZoneMut<P: Ptr> : Zone<P> {
    fn get_mut<'a, T: ?Sized + Load<P>>(&self, ptr: &'a mut ValidPtr<T, P>) -> RefMut<'a, T, P>;
}

pub trait Alloc : Sized {
    type Ptr : Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, Self::Ptr>;
    fn zone(&self) -> <Self::Ptr as Ptr>::Zone;
}

impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, A::Ptr> {
        (**self).alloc(src)
    }

    fn zone(&self) -> <A::Ptr as Ptr>::Zone {
        (**self).zone()
    }
}

