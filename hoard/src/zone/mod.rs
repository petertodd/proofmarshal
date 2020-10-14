use std::ptr::{self, NonNull};
use std::mem::{self, ManuallyDrop};
use std::alloc::Layout;

use crate::pointee::Pointee;
use crate::blob::Blob;
use crate::load::{Load, LoadRef, MaybeValid};
use crate::primitive::Primitive;
use crate::owned::{Take, Ref, IntoOwned, Own};
use crate::bag::Bag;

pub mod heap;

unsafe fn make_bag_from_allocation<T: ?Sized + Pointee, Z, P: Ptr>(
    src: Own<T>,
    dst: NonNull<()>,
    zone_ptr: P,
    zone: Z,
) -> Bag<T, Z, P>
{
    let src: &mut T = Own::leak(src);
    let metadata = T::metadata(src);
    let size = mem::size_of_val(src);

    ptr::copy_nonoverlapping(
        src as *const _ as *const u8,
        dst.as_ptr().cast(),
        size
    );

    Bag::from_raw_parts(zone_ptr, metadata, zone)
}

pub trait Ptr : Sized + FromPtr<Self> + AsPtr<Self> {
    type Clean : PtrConst<Blob = Self::Blob>;
    type Blob : PtrBlob;

    fn from_clean(clean: Self::Clean) -> Self;

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata);
    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Clean>;
    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<&mut T, Self::Clean>;

    unsafe fn try_take_dirty_with<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> R
        where F: FnOnce(Result<Own<T>, Self::Clean>) -> R;

    unsafe fn try_take_dirty<T: ?Sized + Pointee + IntoOwned>(self, metadata: T::Metadata) -> Result<T::Owned, Self::Clean> {
        self.try_take_dirty_with::<T, _, _>(metadata, |src| {
            src.map(T::into_owned)
        })
    }

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, (), Self>
        where Self: Default
    {
        src.take_unsized(|src| {
            let (dst, zone_ptr) = Self::alloc_raw(Layout::for_value::<T>(&src));

            unsafe {
                make_bag_from_allocation(src, dst, zone_ptr, ())
            }
        })
    }

    fn alloc_raw(layout: core::alloc::Layout) -> (NonNull<()>, Self)
        where Self: Default
    {
        Self::alloc_raw_impl(layout)
    }

    fn alloc_raw_impl(layout: core::alloc::Layout) -> (NonNull<()>, Self) {
        unimplemented!()
    }
}

pub trait PtrConst : Copy + FromPtr<Self> + AsPtr<Self> {
    type Blob : PtrBlob;

    fn to_blob(self) -> Self::Blob;
    fn from_blob(blob: Self::Blob) -> Self;
}

pub trait PtrBlob : Primitive + FromPtr<Self> + AsPtr<Self> {
}

impl<P: PtrBlob> PtrConst for P {
    type Blob = Self;

    fn to_blob(self) -> Self::Blob {
        self
    }

    fn from_blob(this: Self) -> Self {
        this
    }
}

impl<P: PtrConst> Ptr for P {
    type Clean = Self;
    type Blob = P::Blob;

    fn from_clean(this: Self) -> Self {
        this
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, _metadata: T::Metadata) {
    }

    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, _metadata: T::Metadata) -> Result<&T, Self> {
        Err(*self)
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, _metadata: T::Metadata) -> Result<&mut T, Self> {
        Err(*self)
    }

    unsafe fn try_take_dirty_with<T: ?Sized + Pointee, F, R>(self, _metadata: T::Metadata, f: F) -> R
        where F: FnOnce(Result<Own<T>, Self>) -> R
    {
        f(Err(self))
    }
}

impl PtrBlob for ! {
    /*
    fn to_blob(self) -> Self {
        match self {}
    }
    */
}

pub trait AsPtr<Q> {
    fn as_ptr(&self) -> &Q;
}

pub trait FromPtr<Q> {
    fn from_ptr(ptr: Q) -> Self;
}

pub trait IntoPtr<Q> {
    fn into_ptr(self) -> Q;
}

impl<P, Q> IntoPtr<Q> for P
where Q: FromPtr<P>
{
    fn into_ptr(self) -> Q {
        Q::from_ptr(self)
    }
}

impl<Q: Ptr> FromPtr<Q> for ! {
    fn from_ptr(_ptr: Q) -> Self {
        panic!()
    }
}

impl<P: Ptr> AsPtr<P> for ! {
    fn as_ptr(&self) -> &P {
        match *self {}
    }
}

pub trait Get<P = <Self as Zone>::Ptr> : Zone {
    unsafe fn get_unchecked<'a, T: ?Sized>(
        &'a self,
        ptr: &'a P,
        metadata: T::Metadata
    ) -> Result<MaybeValid<Ref<'a, T>>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>;

    unsafe fn take_unchecked<T: ?Sized>(
        &self,
        ptr: P,
        metadata: T::Metadata
    ) -> Result<MaybeValid<T::Owned>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>;
}

pub trait GetMut<P = <Self as Zone>::Ptr> : Get<P> {
    unsafe fn get_unchecked_mut<'a, T: ?Sized>(
        &'a self,
        ptr: &'a mut P,
        metadata: T::Metadata
    ) -> Result<MaybeValid<&'a mut T>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>;
}

pub trait Zone : Copy + AsZone<Self> {
    type Error : 'static;
    type Ptr : Ptr;
}

impl Zone for () {
    type Error = !;
    type Ptr = !;
}

pub trait AsZone<Z> {
    fn as_zone(&self) -> &Z;
}

impl<Z: Zone> AsZone<()> for Z {
    fn as_zone(&self) -> &() {
        &()
    }
}

pub trait Alloc : Zone {
    fn alloc_raw(&mut self, layout: core::alloc::Layout) -> (NonNull<()>, Self::Ptr, Self);

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Bag<T, Self, Self::Ptr> {
        src.take_unsized(|src| {
            let (dst, zone_ptr, zone) = self.alloc_raw(Layout::for_value::<T>(&src));

            unsafe {
                make_bag_from_allocation(src, dst, zone_ptr, zone)
            }
        })
    }
}
