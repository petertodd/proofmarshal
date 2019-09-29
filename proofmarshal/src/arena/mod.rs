use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

use std::borrow::Cow;

pub mod heap;
use self::heap::Heap;

pub mod missing;

pub trait Coerce<P> {
    type Coerced : Load<P>;
}

pub trait Ptr : Clone {
    type Error : 'static + fmt::Debug;
    type Allocator : Alloc<Ptr=Self>;

    unsafe fn clone_ptr<T>(&self) -> Own<T,Self>;
    unsafe fn dealloc<T>(self);

    unsafe fn try_get<'p,T: Load<Self>>(&'p self) -> Result<Cow<'p, T>, Self::Error>;
    unsafe fn try_take<T: Load<Self>>(self) -> Result<T, Self::Error>;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        unimplemented!("{} needs to implement Ptr::allocator()", type_name::<Self>())
    }

    unsafe fn debug_get<T>(&self) -> Option<&T> {
        None
    }
}

pub trait MutPtr : Ptr {
    unsafe fn try_get_mut<'p, T: Load<Self>>(&'p mut self) -> Result<&'p mut T, Self::Error>;
}

pub trait Alloc {
    type Ptr : Ptr;

    fn alloc<T>(&mut self, value: T) -> Own<T,Self::Ptr>;
}

pub trait Load<P> : Clone {
    type Error;
}

impl<T: Clone, P> Load<P> for T {
    type Error = !;
}

pub struct Own<T, P: Ptr = Heap> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<P>,
}

impl<T,P: Ptr> Own<T,P> {
    pub fn new(value: T) -> Self
        where P: Default
    {
        Self::new_in(value, &mut P::allocator())
    }

    pub fn new_in(value: T, alloc: &mut impl Alloc<Ptr=P>) -> Self {
        alloc.alloc(value)
    }
}

impl<T: Load<P>, P: Ptr> Own<T,P> {
    pub fn get<'a>(&'a self) -> Cow<'a, T> {
        self.try_get()
            .unwrap()
    }

    pub fn take(self) -> T {
        self.try_take()
            .unwrap()
    }

    pub fn try_get<'p>(&'p self) -> Result<Cow<'p, T>, P::Error> {
        unsafe {
            self.ptr.try_get::<T>()
        }
    }

    pub fn try_take(self) -> Result<T, P::Error> {
        let p = Self::into_raw(self);
        unsafe { p.try_take::<T>() }
    }

}

impl<T, P: Ptr> Own<T,P> {
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
}

impl<T,P: Ptr> Clone for Own<T,P> {
    fn clone(&self) -> Self {
        unsafe {
            self.ptr.clone_ptr::<T>()
        }
    }
}

impl<T,P: Ptr> Drop for Own<T,P> {
    fn drop(&mut self) {
        unsafe {
            let ptr = ManuallyDrop::take(&mut self.ptr);
            ptr.dealloc::<T>()
        }
    }
}

impl<T,P: Ptr> Default for Own<T,P>
where T: Default,
      P: Default,
{
    fn default() -> Self {
        P::allocator().alloc(T::default())
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

impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T>(&mut self, value: T) -> Own<T,Self::Ptr> {
        (**self).alloc(value)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let owned_u8: Own<u8> = Own::new(12u8);

        dbg!(owned_u8);
    }
}
