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

pub trait AsPtr<Q> {
    fn as_ptr(&self) -> &Q;
}

impl<Q> AsPtr<Q> for ! {
    fn as_ptr(&self) -> &Q {
        match *self {}
    }
}

pub trait AsZone<Z> {
    fn as_zone(&self) -> &Z;
}

impl<Z> AsZone<Z> for ! {
    fn as_zone(&self) -> &Z {
        match *self {}
    }
}

pub trait Ptr : Sized + AsPtr<Self> + fmt::Debug {
    type Persist : AsPtr<Self> + Clone + fmt::Debug;
    type PersistZone;

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

pub trait Get<P: Ptr> : Sized {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a P, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<P>;

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: P, metadata: T::Metadata) -> T::Owned
        where T: Load<P>;
}

pub trait GetMut<P: Ptr> : Get<P> {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut P, metadata: T::Metadata) -> &'a mut T
        where T: Load<P>;
}

pub trait TryGet<P: Ptr> : Sized {
    type Error;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a P, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<P>;

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: P, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<P>;
}

pub trait TryGetMut<P: Ptr> : TryGet<P> {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized>(&self, ptr: &'a mut P, metadata: T::Metadata)
        -> Result<&'a mut T, Self::Error>
        where T: Load<P>;
}

impl<P: Ptr, Z> TryGet<P> for Z
where Z: Get<P>
{
    type Error = !;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a P, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<P>
    {
        Ok(self.get_unchecked::<T>(ptr, metadata))
    }

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: P, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<P>
    {
        Ok(self.take_unchecked::<T>(ptr, metadata))
    }
}

impl<P: Ptr, Z> TryGetMut<P> for Z
where Z: GetMut<P>
{
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized>(&self, ptr: &'a mut P, metadata: T::Metadata)
        -> Result<&'a mut T, Self::Error>
        where T: Load<P>
    {
        Ok(self.get_mut_unchecked::<T>(ptr, metadata))
    }
}

pub trait Alloc {
    type Zone;
    type Ptr : Ptr;

    fn zone(&self) -> Self::Zone;

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr>;

    fn alloc_ptr<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Self::Ptr {
        let fat = self.alloc_own(src).into_inner();
        fat.raw
    }
}

impl<A: ?Sized + Alloc> Alloc for &'_ mut A {
    type Zone = A::Zone;
    type Ptr = A::Ptr;

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr> {
        (**self).alloc_own(src)
    }
}

impl<A: ?Sized + Alloc> Alloc for Box<A> {
    type Zone = A::Zone;
    type Ptr = A::Ptr;

    fn zone(&self) -> Self::Zone {
        (**self).zone()
    }

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr> {
        (**self).alloc_own(src)
    }
}

impl Ptr for ! {
    type Persist = !;
    type PersistZone = ();

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
