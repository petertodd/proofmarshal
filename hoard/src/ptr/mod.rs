use std::borrow::Borrow;
use std::ops::{Deref, DerefMut};

use owned::IntoOwned;

use crate::refs::Ref;
use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::*;
use crate::scalar::Scalar;

pub trait AsZone<Z> {
    fn as_zone(&self) -> &Z;
}

impl<Z> AsZone<Z> for ! {
    fn as_zone(&self) -> &Z {
        match *self {}
    }
}

impl AsZone<Self> for () {
    fn as_zone(&self) -> &Self {
        self
    }
}


pub trait AsPersistPtr<Q: PersistPtr> {
    fn as_persist_ptr(&self) -> &Q;
}

pub trait AsPtr<Q: Ptr> : Ptr<Persist: AsPersistPtr<Q::Persist>> {
    fn as_ptr(&self) -> &Q;
}

pub trait AsPtrImpl<Q> {
    fn as_ptr_impl(this: &Self) -> &Q;
}

impl<Q> AsPtrImpl<Q> for ! {
    fn as_ptr_impl(this: &Self) -> &Q {
        match *this {}
    }
}

impl<Q: PersistPtr, P> AsPersistPtr<Q> for P
where P: AsPtrImpl<Q>
{
    fn as_persist_ptr(&self) -> &Q {
        P::as_ptr_impl(self)
    }
}

impl<P: Ptr, Q: Ptr> AsPtr<Q> for P
where P::Persist: AsPersistPtr<Q::Persist>,
      P: AsPtrImpl<Q>
{
    fn as_ptr(&self) -> &Q {
        P::as_ptr_impl(self)
    }
}

pub trait Ptr : Sized + DecodePtr<Self> + AsPtrImpl<Self> {
    type Zone;
    type BlobZone : AsZone<Self::BlobZone> + AsZone<()>;
    type Persist : PersistPtr;
    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata);

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist>;
}

pub trait PersistPtr : Scalar + AsPersistPtr<Self> + AsPtrImpl<Self> {
    type Zone;
    type BlobZone : AsZone<Self::BlobZone> + AsZone<()>;
}

impl PersistPtr for ! {
    type Zone = ();
    type BlobZone = ();
}

impl<P: PersistPtr> Ptr for P {
    type Zone = P::Zone;
    type BlobZone = P::BlobZone;
    type Persist = Self;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        Err(*self)
    }
}

pub trait Get {

    //unsafe fn try_take_unchecked<'a, T: ?Sized + Load<'a, Self>>(&'a self, metadata: T::Metadata) -> T::Loaded;
}

pub trait GetMut : Get {
    //unsafe fn try_get_unchecked<'a, T: ?Sized + Load>(&'a self, metadata: T::Metadata) -> T::LoadedMut;
}


pub trait TryGetPtr<P: Ptr> {
    type Error;

    unsafe fn try_get_ptr_unchecked<'p, 'z: 'p, T: ?Sized + LoadPtr<P>>(&'z self, ptr: &'p P, metadata: T::Metadata)
        -> Result<Ref<'p, T>, Self::Error>;
}

pub trait TryGet : Ptr {
    type Error;

    unsafe fn try_get_unchecked<'a, T: ?Sized + LoadPtr<Self>>(&'a self, metadata: T::Metadata)
        -> Result<Ref<'a, T>, <Self as TryGet>::Error>;
}
