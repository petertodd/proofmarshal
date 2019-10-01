use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use std::borrow::{ToOwned, Borrow, BorrowMut};
use std::io;

use pointee::Pointee;

use verbatim::{Verbatim, PtrEncode, PtrDecode};


mod refs;
pub use self::refs::*;

pub mod heap;
pub mod missing;

/*
mod never;
pub use self::never::NeverAlloc;
*/

pub mod primitive;


pub trait Coerced<P> : Sized {
    type Coerced : Clone;
}

pub trait Coerce<P> {
    type Type : ?Sized + Pointee + ToOwned<Owned=Self::Owned>;
    type Owned : Clone + Borrow<Self::Type> + BorrowMut<Self::Type>;
}

impl<P, T> Coerce<P> for T
where T: Coerced<P>,
{
    type Type = <T as Coerced<P>>::Coerced;
    type Owned = <T as Coerced<P>>::Coerced;
}

/// A type whose values can exist behind a `Ptr`.
pub trait Type<P=()> : 'static + Pointee + Coerce<P> {
    fn cast(_coerced: &Self::Type) -> Self
        where Self: Sized,
    {
        unimplemented!()
    }
}

/// Type erased generic pointer.
pub trait Ptr : Clone {
    type Error : 'static + fmt::Debug;
    type Allocator : Alloc<Ptr=Self>;

    /// Deallocates the pointer.
    unsafe fn dealloc<T: ?Sized + Type<Self>>(self);

    /// Clones the pointer.
    unsafe fn clone_ptr<T: ?Sized + Type<Self>>(&self) -> Own<T,Self>;

    /// Makes an allocator for this pointer.
    ///
    /// This must be implemented for pointers that implement `Default`, as it's used by
    /// `Own::default()` among others to make `#[derive(Default)]` ergonomic. If there is no
    /// logical default value for this pointer type its fine to simply panic.
    ///
    /// The default implementation panics and *must* be overrridden.
    fn allocator() -> Self::Allocator
        where Self: Default
    {
        unimplemented!("{} needs to implement Ptr::allocator()", type_name::<Self>())
    }

    /// Attempts to get the value for debugging purposes.
    ///
    /// Returns `None` by default.
    ///
    /// This must not have side-effects!
    unsafe fn debug_get<T: ?Sized + Type<Self>>(&self) -> Option<&T::Type> {
        None
    }

    unsafe fn verbatim_encode<T, W, Q>(&self, dst: W, ptr_encoder: &mut impl PtrEncode<Q>) -> Result<W, io::Error>
        where Self: Verbatim<Q>,
              W: io::Write,
              T: ?Sized + Type<Self>,
    {
        let _ = (dst, ptr_encoder);
        unimplemented!()
    }

    unsafe fn verbatim_decode<T, Q>(&self, src: &[u8], ptr_decoder: &mut impl PtrDecode<Q>) -> Result<Self, !>
        where Self: Verbatim<Q>,
              T: ?Sized + Type<Self>,
    {
        let _ = (src, ptr_decoder);
        unimplemented!()
    }
}

pub trait TryGet : Ptr {
    unsafe fn try_get<'p,T>(&'p self) -> Result<Ref<'p,T,Self>, Self::Error>
        where T: ?Sized + Type<Self>;

    unsafe fn try_take<T>(self) -> Result<T::Owned, Self::Error>
        where T: ?Sized + Type<Self>;
}

pub trait TryGetMut : TryGet {
    unsafe fn try_get_mut<'p, T>(&'p mut self) -> Result<&'p mut T::Owned, Self::Error>
        where T: ?Sized + Type<Self>;
}

pub trait Get : TryGet {
    unsafe fn get<'p,T>(&'p self) -> Ref<'p,T,Self>
        where T: ?Sized + Type<Self>;

    unsafe fn take<T>(self) -> T::Owned
        where T: ?Sized + Type<Self>;
}

pub trait GetMut : Get + TryGetMut {
    unsafe fn get_owned<'p,T>(&'p self) -> &'p T::Owned
        where T: ?Sized + Type<Self>;

    unsafe fn get_mut<'p, T>(&'p mut self) -> &'p mut T::Owned
        where T: ?Sized + Type<Self>;
}

/// An allocator for a pointer.
pub trait Alloc {
    /// The type of pointer this allocates.
    type Ptr : Ptr;

    /// Allocate a value.
    fn alloc<T>(&mut self, value: T::Owned) -> Own<T,Self::Ptr>
        where T: ?Sized + Type<Self::Ptr>;
}

/// An owned value behind a pointer.
pub struct Own<T: ?Sized + Type<P>, P: Ptr = ()> {
    marker: PhantomData<T::Type>,
    ptr: ManuallyDrop<P>,
}

impl<T: ?Sized + Type<P>, P: Ptr> Own<T,P> {
    pub unsafe fn from_raw(ptr: P) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
        }
    }

    pub fn into_raw(this: Self) -> P {
        let mut this = ManuallyDrop::new(this);

        unsafe { ManuallyDrop::take(&mut this.ptr) }
    }

    /// Creates a new `Own` using the default allocator.
    pub fn new(value: T::Owned) -> Self
        where P: Default
    {
        Self::new_in(value, &mut P::allocator())
    }

    /// Creates a new `Own` using the specified allocator.
    pub fn new_in(value: T::Owned, alloc: &mut impl Alloc<Ptr=P>) -> Self {
        alloc.alloc::<T>(value)
    }

    /// Gets the value.
    ///
    /// May or may not be borrowed, depending on whether or not `P` is an in-memory pointer.
    pub fn get<'a>(&'a self) -> Ref<'a,T,P>
        where P: Get
    {
        unsafe { self.ptr.get::<T>() }
    }

    /// Fallible `get()`
    pub fn try_get<'p>(&'p self) -> Result<Ref<'p,T,P>, P::Error>
        where P: TryGet
    {
        unsafe { self.ptr.try_get::<T>() }
    }

    /// Takes the value.
    pub fn take(self) -> T::Owned
        where P: Get
    {
        let p = Self::into_raw(self);
        unsafe { p.take::<T>() }
    }

    /// Fallible `take()`
    pub fn try_take(self) -> Result<T::Owned, P::Error>
        where P: TryGet
    {
        let p = Self::into_raw(self);
        unsafe { p.try_take::<T>() }
    }

    pub fn get_mut(&mut self) -> &mut T::Owned
        where P: GetMut
    {
        unsafe {
            self.ptr.get_mut::<T>()
        }
    }

    /// Fallible `get_mut()`.
    pub fn try_get_mut(&mut self) -> Result<&mut T::Owned, P::Error>
        where P: TryGetMut
    {
        unsafe {
            self.ptr.try_get_mut::<T>()
        }
    }
}

impl<T: ?Sized + Type<P>, P: Ptr> fmt::Debug for Own<T,P>
where T::Type: fmt::Debug,
      P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match unsafe { self.ptr.debug_get::<T>() } {
            Some(r) => r.fmt(f),
            None => f.debug_tuple(type_name::<Self>())
                     .field(&self.ptr)
                     .finish()
        }
    }
}


impl<T: ?Sized + Type<P>, P: Ptr> Clone for Own<T,P> {
    fn clone(&self) -> Self {
        unsafe {
            self.ptr.clone_ptr::<T>()
        }
    }
}

impl<T: ?Sized + Type<P>, P: Ptr> Drop for Own<T,P> {
    fn drop(&mut self) {
        unsafe {
            let ptr = ManuallyDrop::take(&mut self.ptr);
            ptr.dealloc::<T>()
        }
    }
}

impl<T: ?Sized + Type<P>, P: Ptr> Default for Own<T,P>
where T::Owned: Default,
      P: Default,
{
    fn default() -> Self {
        P::allocator().alloc(T::Owned::default())
    }
}


impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T>(&mut self, value: T::Owned) -> Own<T,Self::Ptr>
        where T: ?Sized + Type<Self::Ptr>
    {
        (**self).alloc(value)
    }
}

impl<T: ?Sized + Type<P>, P: Ptr, Q: Ptr> Coerced<Q> for Own<T,P>
where T: Type<Q>
{
    type Coerced = Own<T,Q>;
}

impl<T: ?Sized + Type + Type<P>, P: Ptr> Type<P> for Own<T>
{
    fn cast(_coerced: &<Self as Coerce<P>>::Type) -> Self
        where Self: Sized,
    {
        unsafe { Own::from_raw(()) }
    }
}

impl<T: Type<P>, P: Ptr, Q> Verbatim<Q> for Own<T,P>
where P: Verbatim<Q>
{
    type Error = !;

    const LEN: usize = P::LEN;
    const NONZERO_NICHE: bool = P::NONZERO_NICHE;

    fn encode<W: io::Write>(&self, _dst: W, _ptr_encoder: &mut impl PtrEncode<Q>) -> Result<W, io::Error> {
        unimplemented!()
    }

    fn decode(_src: &[u8], _ptr_decoder: &mut impl PtrDecode<Q>) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::{*, heap::Heap};

    #[test]
    fn test() {
        let owned_u8: Own<u8,Heap> = Own::new(12u8);

        let own2 = Own::<Own<u8>, Heap>::new(owned_u8);

        assert_eq!(*own2.get().get(), 12);
    }
}
