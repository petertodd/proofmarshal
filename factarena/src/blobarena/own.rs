//! Owned values in arenas.

use core::any::type_name;
use core::marker::PhantomData;

use crate::pointee::Pointee;
use crate::ptr::Ptr;

use super::*;

/// An owned value in an arena.
pub struct Own<T: ?Sized + Pointee, A: Arena> {
    marker: PhantomData<T>,

    pub(crate) ptr: Ptr<T,A::Ptr>,
    pub(crate) arena: A,
}

impl<T: ?Sized + Load<A>, A: Arena> Own<T,A> {
    pub fn try_get<'a>(&'a self) -> Result<Ref<'a, T>, A::Error> {
        self.arena.try_deref_ptr(&self.ptr)
    }

    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where A: Avail
    {
        self.arena.deref_ptr(&self.ptr)
    }
}

impl<T: ?Sized + Pointee, A: Arena> Own<T,A> {
    #[inline]
    pub fn from_ptr(ptr: Ptr<T,A::Ptr>, arena: A) -> Self {
        Self {
            marker: PhantomData,
            ptr, arena,
        }
    }
}

impl<T: Pointee, A: Global> Own<T,A>
where T: Store<A>
{
    pub fn new(value: T) -> Self {
        A::allocator().alloc(value)
    }
}

impl<A: Arena, T: Store<A>> Own<T,A> {
    pub fn new_in(value: T, allocator: &mut impl Alloc<A>) -> Self {
        allocator.alloc(value)
    }
}

impl<T: ?Sized + Pointee, A: Arena> fmt::Debug for Own<T,A>
where T: fmt::Debug,
      A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.arena.deref_mem_ptr(&self.ptr) {
            Some(v) => v.fmt(f),
            None => {
                f.debug_struct(type_name::<Self>())
                    .field("ptr", &self.ptr)
                    .field("arena", &self.arena)
                    .finish()
            }
        }
    }
}

impl<T: ?Sized + Store<A>, A: ?Sized + Arena> Default for Own<T,A>
where T: Default,
      A: Global,
{
    fn default() -> Self {
        A::allocator().alloc(T::default())
    }
}

/*
impl<T: ?Sized + Load<A>, A: ?Sized + Arena> Clone for Own<T,A>
where T: Clone,
      A: Clone,
{
    fn clone(&self) -> Self {
        let new_ptr = self.arena.clone_ptr(&self.ptr);
        Own::from_ptr(new_ptr, self.arena.clone())
    }
}

*/
