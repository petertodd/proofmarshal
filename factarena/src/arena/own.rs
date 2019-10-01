/// Generic pointers

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

use super::{Pointee, Arena, Locate, Alloc, marshal::Type};

/// Owned value in an arena.
#[repr(C)]
pub struct Own<T: ?Sized + Pointee, A: Arena> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<A::Ptr>,
    metadata: T::Metadata,
}

impl<T: ?Sized + Pointee, A: Arena> Drop for Own<T,A> {
    fn drop(&mut self) {
        let ptr = unsafe { ManuallyDrop::take(&mut self.ptr) };
        unsafe {
            A::dealloc::<T>(ptr, self.metadata)
        }
    }
}

impl<T: Type<A>, A: Locate> Own<T,A> {
    #[inline]
    pub fn new_in(value: T, mut allocator: impl Alloc<Arena=A>) -> Self {
        allocator.alloc(value)
    }
}

impl<T: ?Sized + Pointee, A: Arena> Own<T,A> {
    /// Creates a new `Own<T,A>`.
    ///
    /// # Safety
    ///
    /// You are asserting that the pointer is appropriate for the declared type `T`.
    pub unsafe fn from_raw(ptr: A::Ptr, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
            metadata,
        }
    }

    /// Deconstructs the `Own` into its raw parts.
    pub fn into_raw(self) -> (A::Ptr, T::Metadata) {
        let mut this = ManuallyDrop::new(self);
        let ptr = unsafe { ManuallyDrop::take(&mut this.ptr) };
        (ptr, this.metadata)
    }

    /// Accesses the underlying raw pointer.
    pub fn ptr(&self) -> &A::Ptr {
        &*self.ptr
    }

    /// Gets the metadata.
    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<T: ?Sized + Pointee, A: Arena> fmt::Debug for Own<T,A>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match unsafe { A::debug_deref::<T>(&self.ptr, self.metadata) } {
            Some(r) => r.fmt(f),
            None => f.debug_struct(type_name::<Self>())
                        .field("ptr", &self.ptr)
                        .field("metadata", &self.metadata)
                        .finish(),
        }
    }
}

/*
impl<T: ?Sized + Type<A>, A: Arena> Clone for Own<T,A>
where T: Clone,
      A: Clone + Default,
{
    fn clone(&self) -> Self {
        let arena = A::default();
        arena.clone_own(self)
    }
}
*/

/*
impl<T: ?Sized + Pointee, R: Arena> cmp::PartialEq for Ptr<T,R>
where R: cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.metadata.eq(&other.metadata) && self.raw.eq(&other.raw)
    }
}
impl<T: ?Sized + Pointee, R: Arena> cmp::Eq for Ptr<T,R>
where R: cmp::Eq {}


/// Generic missing pointer
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

impl Arena for Missing {
    unsafe fn dealloc<T: ?Sized + Pointee>(self, _: T::Metadata) {}
}*/
