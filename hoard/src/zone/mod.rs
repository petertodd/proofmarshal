//! Zones where data can be stored.

use std::fmt;
use std::mem::ManuallyDrop;

use owned::Take;

use crate::pointee::Pointee;
use crate::load::Load;
use crate::blob::Persist;
use crate::refs::*;
use crate::ptr::*;
use crate::bag::Bag;

pub trait Zone : Sized {
    type Ptr : Ptr;
}

impl Zone for () {
    type Ptr = !;
}

pub trait Get<P> {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a P, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<Self>;

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: P, metadata: T::Metadata) -> T::Owned
        where T: Load<Self>;
}

pub trait GetMut<P> : Get<P> {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut P, metadata: T::Metadata) -> &'a mut T
        where T: Load<Self>;
}

pub trait TryGet<P> {
    type Error : std::error::Error;

    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a P, metadata: T::Metadata) -> Result<Ref<'a, T>, Self::Error>
        where T: ?Sized + Load<Self>;

    unsafe fn try_take_unchecked<'a, T>(&self, ptr: P, metadata: T::Metadata) -> Result<T::Owned, Self::Error>
        where T: ?Sized + Load<Self>;
}

pub trait TryGetMut<P> : TryGet<P> {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized>(&self, ptr: &'a mut P, metadata: T::Metadata) -> Result<&'a mut T, Self::Error>
        where T: Load<Self>;
}

pub trait Alloc : Sized {
    type Ptr : Ptr;

    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> Bag<T, Self, Self::Ptr>;
}
