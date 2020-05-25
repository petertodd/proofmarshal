use std::cmp;
use std::fmt;
use std::mem::ManuallyDrop;

use owned::{Take, IntoOwned};

use crate::pointee::Pointee;
use crate::load::Load;
use crate::refs::Ref;

mod error;
pub use self::error::*;

pub mod fat;
pub use self::fat::Fat;

pub mod own;
pub use self::own::Own;

pub trait Ptr : Sized + fmt::Debug {
    type Persist : 'static + Clone + fmt::Debug;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata);

    fn duplicate(&self) -> Self;

    unsafe fn clone_unchecked_with<T, U, F>(&self, metadata: T::Metadata, f: F) -> Own<T, Self>
        where T: ?Sized + Pointee,
              F: FnOnce(&T) -> U,
              U: Take<T>,
              Self: Clone
    {
        unimplemented!()
    }

    unsafe fn clone_unchecked<T>(&self, metadata: T::Metadata) -> Own<T, Self>
        where T: ?Sized + Pointee + ToOwned,
              T::Owned: Take<T>,
              Self: Clone
    {
        self.clone_unchecked_with(metadata, T::to_owned)
    }

    fn alloc<T: ?Sized + Pointee, U: Take<T>>(src: U) -> Own<T, Self>
        where Self: Default
    {
        unimplemented!()
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist>;
    unsafe fn try_take_dirty_unchecked<T: ?Sized + Pointee>(self, metadata: T::Metadata) -> Result<T::Owned, Self::Persist>
        where T: IntoOwned;
}

impl Ptr for ! {
    type Persist = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
        match *self {}
    }
    fn duplicate(&self) -> Self { *self }

    unsafe fn clone_unchecked_with<T, U, F>(&self, _: T::Metadata, _: F) -> Own<T, Self>
        where T: ?Sized + Pointee
    {
        match *self {}
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        match *self {}
    }
    unsafe fn try_take_dirty_unchecked<T: ?Sized + Pointee>(self, _: T::Metadata) -> Result<T::Owned, Self::Persist>
        where T: IntoOwned
    {
        match self {}
    }
}

pub trait Get<P: Ptr> : Sized {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a P, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<Self>;

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: P, metadata: T::Metadata) -> T::Owned
        where T: Load<Self>;
}

pub trait GetMut<P: Ptr> : Get<P> {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut P, metadata: T::Metadata) -> &'a mut T
        where T: Load<Self>;
}

pub trait TryGet<P: Ptr> : Sized {
    type Error;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a P, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<Self>;

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: P, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<Self>;
}

pub trait TryGetMut<P: Ptr> : TryGet<P> {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized>(&self, ptr: &'a mut P, metadata: T::Metadata)
        -> Result<&'a mut T, Self::Error>
        where T: Load<Self>;
}

impl<P: Ptr, Z> TryGet<P> for Z
where Z: Get<P>
{
    type Error = !;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a P, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<Self>
    {
        Ok(self.get_unchecked::<T>(ptr, metadata))
    }

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: P, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<Self>
    {
        Ok(self.take_unchecked::<T>(ptr, metadata))
    }
}

impl<P: Ptr, Z> TryGetMut<P> for Z
where Z: GetMut<P>
{
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized>(&self, ptr: &'a mut P, metadata: T::Metadata)
        -> Result<&'a mut T, Self::Error>
        where T: Load<Self>
    {
        Ok(self.get_mut_unchecked::<T>(ptr, metadata))
    }
}

pub trait Alloc : Sized {
    type Zone;
    type Ptr : Ptr;

    fn zone(&self) -> Self::Zone;

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr>;
}

/*
impl<A: ?Sized + Alloc> Alloc for &'_ mut A {
    type Zone = A::Zone;
    type Ptr = A::Ptr;

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }

    fn alloc<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> FatPtr<T, Self> {
        (**self).alloc(src)
    }
}

impl<A: ?Sized + Alloc> Alloc for Box<A> {
    type Zone = A::Zone;
    type Ptr = A::Ptr;

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }

    fn alloc<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> FatPtr<T, Self> {
        (**self).alloc(src)
    }
}
*/

/*
pub trait AsZone<Y> {
    fn as_zone(&self) -> &Y;
}

impl<Z: ?Sized> AsZone<()> for Z {
    fn as_zone(&self) -> &() {
        &()
    }
}

pub trait AsPtr<Q> {
    fn as_ptr(&self) -> &Q;
}

impl<P> AsPtr<!> for P {
    fn as_ptr(&self) -> &! {
        panic!()
    }
}

impl<P> AsPtr<()> for P {
    fn as_ptr(&self) -> &() {
        &()
    }
}

impl Ptr for ! {
    type Zone = ();
    type ZoneError = !;
    type Persist = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
        match *self {}
    }

    fn zone() -> Self::Zone {}

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        match *self {}
    }

    fn duplicate(&self) -> Self {
        match *self {}
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        match *self {}
    }
}
*/
