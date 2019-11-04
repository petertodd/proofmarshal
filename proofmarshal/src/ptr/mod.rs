//! Generic pointers.
//!
//! Basic idea: this is meant to seamlessly integrate both volatile and persistent storage.
//! This is why the various `get()` methods on pointers return `Cow`: depending on the type of
//! pointer values may or may not have to be loaded into memory. Secondly, this is why types must
//! be `Clone`: code using this functionality - especially container types - needs to be able to
//! return owned values to callees.

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use std::borrow::Cow;

//mod refs;
//pub use self::refs::*;

pub mod heap;
pub mod missing;

/*
mod never;
pub use self::never::NeverAlloc;
*/

//pub mod primitive;

/// An in-memory pointer.
///
/// Always associated with a lifetime.
///
/// `Mem<A>` can implement all the useful traits, independent of whether or not `A` does.
#[repr(transparent)]
pub struct Mem<A> {
    arena: PhantomData<A>,
    pub raw: NonNull<()>,
}

/// Persistent byte blob.
#[repr(transparent)]
pub struct Blob<A: blob::Arena> {
    /// Entirely invariant?
    arena: PhantomData<fn(A) -> A)>,
    pub offset: A::Offset,
}

/// Rich object protocol.
#[repr(transparent)]
pub struct Obj<A: obj::Arena> {
    /// Entirely invariant?
    arena: PhantomData<fn(A) -> A>,
    pub handle: A::Handle,
}

/// Anything can be loaded from a mem pointer, because it's just a dereference.
impl<T: ?Sized, P: MemPtr> Load<Mem<P>> for T {
}

/// All sized types know how to store; trivial.
impl<T, P: MemPtr> Store<Mem<P>> for T {
}

impl<P: MemPtr> Store<Mem<P>> for Foo<Mem<P>> {
}

impl<A> Ptr for Mem<A> {
    type Arena = A;
}

impl<A> Ptr for Blob<A> {
    type Arena = A;
}

impl<A> Ptr for Obj<A> {
    type Arena = A;
}

pub trait Ptr {
    /// The arena has to know how to deallocate.
    type Arena : Dealloc<Self>;
}

/// Something that knows how to deallocate a type of pointer.
pub trait Dealloc<P> {
    fn dealloc<T>(&mut self, ptr: P, metadata: T::Metadata)
        where T: ?Sized + Pointee;
}


/// Type erased generic pointer.
pub trait Ptr : Clone {
    type Arena;

    type Error : 'static + fmt::Debug;
    type Allocator : Alloc<Ptr=Self>;

    /// Deallocates a sized value.
    unsafe fn dealloc<T>(self);

    /// Clones the pointer.
    unsafe fn clone_ptr<T>(&self) -> Own<T,Self>;

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
    unsafe fn debug_get<T>(&self) -> Option<&T> {
        None
    }

    /*
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
    */
}

pub trait Load<P> : Clone {
}

impl<T: Clone, P> Load<P> for T {}

pub trait Store<P> : Clone {
}
impl<T: Clone, P> Store<P> for T {}

/// Fallible dereferencing.
pub trait TryGet : Ptr {
    unsafe fn try_get<'p,T>(&'p self) -> Result<Cow<'p,T>, Self::Error>
        where T: Load<Self>;

    unsafe fn try_take<T>(self) -> Result<T, Self::Error>
        where T: Load<Self>;
}

/// Fallible mutable dereferencing.
pub trait TryGetMut : TryGet {
    unsafe fn try_get_mut<T>(&mut self) -> Result<&mut T, Self::Error>
        where T: Load<Self>;
}

/// Infallible version of `TryGet`.
pub trait Get : TryGet {
    unsafe fn get<'p,T>(&'p self) -> Cow<'p,T>
        where T: Load<Self>;

    unsafe fn take<T>(self) -> T
        where T: Load<Self>;
}

/// Infallible version of `TryGetMut`.
pub trait GetMut : Get + TryGetMut {
    unsafe fn get_mut<T>(&mut self) -> &mut T
        where T: Load<Self>;
}

/// An allocator for a pointer.
pub trait Alloc {
    /// The type of pointer this allocates.
    type Ptr : Ptr;

    /// Allocate a value.
    fn alloc<T>(&mut self, value: T) -> Own<T,Self::Ptr>
        where T: Store<Self::Ptr>;
}

/// An owned value behind a pointer.
pub struct Own<T, P: Ptr> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<P>,
}


impl<T, P: Ptr> Own<T,P>
where T: Store<P>
{
    /// Creates a new `Own` using the default allocator.
    pub fn new(value: T) -> Self
        where P: Default
    {
        Self::new_in(value, &mut P::allocator())
    }

    /// Creates a new `Own` using the specified allocator.
    pub fn new_in(value: T, alloc: &mut impl Alloc<Ptr=P>) -> Self {
        alloc.alloc::<T>(value)
    }
}

impl<T, P: Ptr> Own<T,P>
where T: Load<P>
{
    /// Gets the value.
    ///
    /// May or may not be borrowed, depending on whether or not `P` is an in-memory pointer.
    pub fn get<'a>(&'a self) -> Cow<'a,T>
        where P: Get
    {
        unsafe { self.ptr.get::<T>() }
    }

    /// Fallible `get()`
    pub fn try_get<'p>(&'p self) -> Result<Cow<'p,T>, P::Error>
        where P: TryGet
    {
        unsafe { self.ptr.try_get::<T>() }
    }

    /// Takes the value.
    pub fn take(self) -> T
        where P: Get
    {
        let p = Self::into_raw(self);
        unsafe { p.take::<T>() }
    }

    /// Fallible `take()`
    pub fn try_take(self) -> Result<T, P::Error>
        where P: TryGet
    {
        let p = Self::into_raw(self);
        unsafe { p.try_take::<T>() }
    }

    pub fn get_mut(&mut self) -> &mut T
        where P: GetMut
    {
        unsafe {
            self.ptr.get_mut::<T>()
        }
    }

    /// Fallible `get_mut()`.
    pub fn try_get_mut(&mut self) -> Result<&mut T, P::Error>
        where P: TryGetMut
    {
        unsafe {
            self.ptr.try_get_mut::<T>()
        }
    }
}

impl<T, P: Ptr> Own<T,P> {
    /// Unsafely creates a new `Own` from a raw pointer.
    pub unsafe fn from_raw(ptr: P) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
        }
    }

    /// Converts the `Own` into the raw pointer.
    pub fn into_raw(this: Self) -> P {
        let mut this = ManuallyDrop::new(this);

        unsafe { ManuallyDrop::take(&mut this.ptr) }
    }

    pub fn debug_get(&self) -> Option<&T> {
        unsafe { self.ptr.debug_get::<T>() }
    }
}


impl<T, P: Ptr> fmt::Debug for Own<T,P>
where T: fmt::Debug,
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


impl<T, P: Ptr> Clone for Own<T,P> {
    fn clone(&self) -> Self {
        unsafe {
            self.ptr.clone_ptr::<T>()
        }
    }
}

impl<T, P: Ptr> Drop for Own<T,P> {
    fn drop(&mut self) {
        unsafe {
            let ptr = ManuallyDrop::take(&mut self.ptr);
            ptr.dealloc::<T>()
        }
    }
}

impl<T: Store<P>, P: Ptr> Default for Own<T,P>
where T: Default,
      P: Default,
{
    fn default() -> Self {
        P::allocator().alloc(T::default())
    }
}


impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T>(&mut self, value: T) -> Own<T,Self::Ptr>
        where T: Store<Self::Ptr>,
    {
        (**self).alloc(value)
    }
}

/*
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
*/

#[cfg(test)]
mod tests {
    use super::{*, heap::Heap};

    #[test]
    fn test() {
        let owned_u8: Own<u8,Heap> = Own::new(12u8);

        let own2 = Own::<Own<u8,Heap>, Heap>::new(owned_u8);

        assert_eq!(*own2.get().get(), 12);
    }
}
