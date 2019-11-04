//! Zones

#![feature(box_into_raw_non_null)]
#![feature(manually_drop_take)]
#![feature(never_type)]

use core::borrow::Borrow;
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

use pointee::Pointee;
//use owned::{Owned, Ref};

pub mod never;
pub mod heap;

mod refs;
pub use self::refs::*;

pub mod pile;
pub mod hoard;

pub trait Zone : Sized {
    /// Raw pointer type.
    type Ptr : fmt::Debug + Eq + Ord + hash::Hash;

    /// Default allocator for this zone.
    type Allocator : Alloc<Zone=Self>;

    /// Error returned when a pointer can't be loaded.
    ///
    /// Specifically, this is when the pointer itself is invalid, rather than the value behind the
    /// pointer.
    type Error : fmt::Debug;

    /// Creates a new allocator for this zone.
    ///
    /// Any zone implementing `Default` is expected to implement this to allow `#[derive(Default)`
    /// to work.
    fn allocator() -> Self::Allocator
        where Self: Default
    {
        unimplemented!()
    }

    /// Clones a record in this zone.
    ///
    /// Any zone implementing `Clone` is expected to implement this to allow `#[derive(Clone)`
    /// to work.
    fn clone_rec<T: Clone>(r: &Rec<T,Self>) -> Rec<T,Self>
        where Self: Clone
    {
        let _ = r;
        unimplemented!()
    }

    /// Deallocates a uniquely owned pointer.
    ///
    /// Note how this is an associated function: to reduce the amount of code that needs to deal
    /// with deallocation - and thus the risk of memory leaks - zones are expected to be able to
    /// perform deallocation without access to the zone object itself.
    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T,Self>);
}

pub trait Get : Zone {
    fn get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Ref<'p, T, Self>;
}

/*
pub trait GetMut : Zone {
    fn get_mut<'p, T: ?Sized + Load<Self>>(&self, r: &'p mut Rec<T,Self>) -> &'p mut T;
}
*/

pub trait Load<Z: Zone> : Pointee {
    type Error : fmt::Debug;
    type Owned : Borrow<Self>;

    fn load<'p>(zone: &Z, r: &'p Rec<Self,Z>) -> Result<Ref<'p, Self, Z>, Self::Error>;
}

pub trait Store<Z: Zone> : Pointee {
    fn store(self, allocator: &mut Z::Allocator) -> Rec<Self,Z>;
}

/// An allocator for a zone.
pub trait Alloc {
    type Zone : Zone;

    fn alloc<T: Store<Self::Zone>>(&mut self, value: T) -> Rec<T,Self::Zone>;

    /// Creates a new zone handle.
    fn zone(&self) -> Self::Zone;
}

/// The ability to move to a different zone.
pub trait Coerce<A: Zone> : Sized {
    type Coerced : Sized;

    fn coerce_in(self, alloc: &mut impl Alloc<Zone=A>) -> Self::Coerced;

    fn coerce(self) -> Self::Coerced
        where A: Default
    {
        self.coerce_in(&mut A::allocator())
    }
}

/// Pointer to a value in an zone.
///
/// *Not* guaranteed valid.
pub struct Ptr<T: ?Sized + Pointee, Z: Zone> {
    pub raw: Z::Ptr,
    pub metadata: T::Metadata,
}

/// Record in an zone.
pub struct Rec<T: ?Sized + Pointee, Z: Zone> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<Ptr<T,Z>>,
}

/// Owned value and zone; the zone equivalent of a `Box`.
pub struct Bag<T: ?Sized + Pointee, Z: Zone> {
    rec: Rec<T,Z>,
    zone: Z,
}

impl<T: ?Sized + Pointee, A: Zone> Drop for Rec<T,A> {
    fn drop(&mut self) {
        unsafe {
            let ptr = ManuallyDrop::take(&mut self.ptr);
            A::dealloc(ptr);
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Rec<T,Z> {
    /// Creates a `Rec` from the raw pointer and metadata.
    ///
    /// # Safety
    ///
    /// The pointer and metadata must be valid.
    pub unsafe fn from_ptr(ptr: Ptr<T,Z>) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
        }
    }

    /// Consumes the `Own`, returning the raw pointer and metadata.
    pub fn into_ptr(self) -> Ptr<T,Z> {
        unsafe {
            let mut this = ManuallyDrop::new(self);
            ManuallyDrop::take(&mut this.ptr)
        }
    }

    /// Gets the underlying pointer.
    #[inline(always)]
    pub fn ptr(&self) -> &Ptr<T,Z> {
        &self.ptr
    }
}


impl<T: Store<Z>, Z: Zone> Bag<T,Z> {
    pub fn new(value: T) -> Self
        where Z: Default,
    {
        Self::new_in(value, &mut Z::allocator())
    }

    pub fn new_in(value: T, allocator: &mut impl Alloc<Zone=Z>) -> Self {
        Self::from_raw_parts(allocator.alloc(value), allocator.zone())
    }
}

impl<T: ?Sized + Load<Z>, Z: Get> Bag<T,Z> {
    pub fn get(&self) -> Ref<T,Z> {
        self.zone.get(&self.rec)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Bag<T,Z> {
    /// Creates an `Bag` from a record and zone.
    pub fn from_raw_parts(rec: Rec<T,Z>, zone: Z) -> Self {
        Self { rec, zone, }
    }

    /// Consumes the `Bag`, returning the wrapped record and zone.
    pub fn into_raw_parts(this: Self) -> (Rec<T,Z>, Z) {
        (this.rec, this.zone)
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
*/
