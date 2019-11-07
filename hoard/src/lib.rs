//! Zones

#![feature(box_into_raw_non_null)]
#![feature(manually_drop_take)]
#![feature(never_type)]

use core::borrow::Borrow;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

use pointee::Pointee;
//use owned::{Owned, Ref};

pub mod never;
pub mod heap;

pub mod pile;
pub mod marshal;

mod refs;
pub use self::refs::*;

pub mod linkedlist;

pub trait Zone : Sized {
    /// Raw pointer type.
    type Ptr : fmt::Debug + Eq + Ord;

    /// Default allocator for this zone.
    type Allocator : Alloc<Zone=Self>;

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
    fn clone_rec<T: Clone>(r: &Rec<T,Self>) -> Rec<T,Self>
        where Self: Clone;

    /// Deallocates a uniquely owned pointer.
    ///
    /// Note how this is an associated function: to reduce the amount of code that needs to deal
    /// with deallocation - and thus the risk of memory leaks - zones are expected to be able to
    /// perform deallocation without access to the zone object itself.
    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T,Self>);

    fn fmt_debug_rec<T: ?Sized + Pointee>(rec: &Rec<T,Self>, f: &mut fmt::Formatter) -> fmt::Result
        where T: fmt::Debug;

    fn fmt_debug<T: ?Sized + Pointee>(&self, rec: &Rec<T,Self>, f: &mut fmt::Formatter) -> fmt::Result
        where T: fmt::Debug
    {
        Self::fmt_debug_rec(rec, f)
    }
}

pub trait Load<Z: Zone> : Pointee {
    type Error : fmt::Debug;
    type Owned : Borrow<Self>;

    unsafe fn take(borrowed: &mut ManuallyDrop<Self>) -> Self::Owned;

    fn pile_load<'p, L>(pile: &Z, rec: &'p Rec<Self, Z>) -> Result<Result<Ref<'p, Self, Z>, Self::Error>, Z::Error>
        where Z: pile::Pile;
}

pub trait Store<Z: Zone> : Load<Z> {
    unsafe fn alloc(owned: Self::Owned, dst: *mut ()) -> *mut Self;

    fn pile_store<D: pile::Dumper<Pile=Z>>(owned: Self::Owned, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile;
}

pub trait TryGet : Zone {
    fn try_get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Result<Ref<'p, T, Self>, Self::Error>;
}

pub trait TryTake : TryGet {
    fn try_take<T: ?Sized + Load<Self>>(&self, r: Rec<T,Self>) -> Result<T::Owned, Self::Error>;
}

pub trait Get : Zone {
    fn get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Ref<'p, T, Self>;
}

pub trait Take : Get {
    fn take<T: ?Sized + Load<Self>>(&self, r: Rec<T,Self>) -> T::Owned;
}

impl<Z: TryGet + Zone<Error=!>> Get for Z {
    #[inline(always)]
    fn get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Ref<'p, T, Self> {
        match self.try_get(r) {
            Ok(r) => r,
            Err(never) => never,
        }
    }
}

impl<Z: TryTake + Zone<Error=!>> Take for Z {
    #[inline(always)]
    fn take<T: ?Sized + Load<Self>>(&self, r: Rec<T,Self>) -> T::Owned {
        match self.try_take(r) {
            Ok(owned) => owned,
            Err(never) => never,
        }
    }
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
#[derive(Clone)]
pub struct Bag<T: ?Sized + Pointee, Z: Zone> {
    rec: Rec<T,Z>,
    zone: Z,
}

impl<T: ?Sized + Pointee, Z: Zone> Drop for Rec<T,Z> {
    fn drop(&mut self) {
        unsafe {
            let ptr = ManuallyDrop::take(&mut self.ptr);
            Z::dealloc(ptr);
        }
    }
}

impl<T: Clone, Z: Zone + Clone> Clone for Rec<T,Z> {
    #[inline]
    fn clone(&self) -> Self {
        Z::clone_rec(self)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for Rec<T,Z>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Z::fmt_debug_rec(self, f)
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

impl<T: ?Sized + Load<Z>, Z: Zone> Bag<T,Z> {
    pub fn get<'p>(&'p self) -> Ref<'p, T, Z>
        where Z: Get
    {
        self.zone.get(&self.rec)
    }

    pub fn try_get<'p>(&'p self) -> Result<Ref<'p, T, Z>, Z::Error>
        where Z: TryGet
    {
        self.zone.try_get(&self.rec)
    }

    pub fn take(self) -> T::Owned
        where Z: Take
    {
        self.zone.take(self.rec)
    }

    pub fn try_take(self) -> Result<T::Owned, Z::Error>
        where Z: TryTake
    {
        self.zone.try_take(self.rec)
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

impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for Bag<T,Z>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.zone.fmt_debug(&self.rec, f)
    }
}

impl<T, Z: Zone> Default for Bag<T,Z>
where T: Default + Store<Z>,
      Z: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use heap::Heap;

    #[test]
    fn test() {
        let b = Bag::<u8,Heap>::new(42u8);

        dbg!(b);
    }
}
