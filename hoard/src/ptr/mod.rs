//! Copy-on-write pointer functionality, for data that may be stored on disk.

use std::fmt;

use crate::blob::Blob;
use crate::pointee::Pointee;
use crate::validate::MaybeValid;
use crate::owned::{Take, IntoOwned, Ref, RefOwn};
use crate::bag::Bag;
use crate::load::LoadRef;

pub mod heap;
pub use self::heap::Heap;

pub mod key;
pub use self::key::Key;

//pub mod cow;
//pub use self::cow::Cow;

pub mod error;
pub use self::error::{Error, ErrorKind};


pub trait AsZone<Z: ?Sized> {
    fn as_zone(&self) -> &Z;
}

impl<T: ?Sized> AsZone<()> for &'_ T {
    fn as_zone(&self) -> &() {
        &()
    }
}

impl<T: ?Sized> AsZone<T> for T {
    fn as_zone(&self) -> &Self {
        self
    }
}

pub trait Zone : Copy + AsZone<()> {
    type Id : 'static + Send;
}

impl Zone for () {
    type Id = ();
}

pub trait Ptr : Sized {
    type Zone : Zone;
    type Clean : PtrClean<Zone = Self::Zone, Blob = Self::Blob>;
    type Blob : PtrBlob;

    fn from_clean(clean: Self::Clean) -> Self;

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata);
    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<MaybeValid<&T>, Self::Clean>;
    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Clean>;
    unsafe fn try_take_dirty_then<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Clean>
        where F: FnOnce(MaybeValid<RefOwn<T>>) -> R;

    unsafe fn try_take_dirty<T: ?Sized>(self, metadata: T::Metadata) -> Result<MaybeValid<T::Owned>, Self::Clean>
        where T: Pointee + IntoOwned
    {
        self.try_take_dirty_then::<T, _, _>(metadata, |src| {
            T::into_owned(src.trust()).into()
        })
    }

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self>
        where Self: Default
    {
        unimplemented!()
    }
}

/// Needs no deallocation; data available.
pub trait PtrClean : Copy {
    type Zone : Zone;
    type Blob : PtrBlob;

    fn zone(&self) -> Self::Zone;
    fn to_blob(self) -> Self::Blob;
    fn from_blob(blob: Self::Blob, zone: &Self::Zone) -> Self;
}

impl<P: PtrClean> Ptr for P {
    type Zone = P::Zone;
    type Clean = Self;
    type Blob = P::Blob;

    fn from_clean(this: Self) -> Self {
        this
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, _metadata: T::Metadata) {
    }

    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, _metadata: T::Metadata) -> Result<MaybeValid<&T>, Self::Clean> {
        Err(*self)
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, _metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Clean> {
        Err(*self)
    }

    unsafe fn try_take_dirty_then<T: ?Sized + Pointee, F, R>(self, _metadata: T::Metadata, f: F) -> Result<R, Self::Clean>
        where F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        Err(self)
    }
}

/// Raw blob.
pub trait PtrBlob : Copy + Blob {
}

impl<P: PtrBlob> PtrClean for P {
    type Zone = ();
    type Blob = Self;

    fn zone(&self) -> () {
    }

    fn to_blob(self) -> Self {
        self
    }

    fn from_blob(this: Self, _zone: &()) -> Self {
        this
    }
}

impl PtrBlob for ! {}
impl PtrBlob for () {}

pub trait TryGet : Ptr {
    type Error;

    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>;

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R;

    unsafe fn try_take<T: ?Sized>(self, metadata: T::Metadata) -> Result<MaybeValid<T::Owned>, Self::Error>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>,
    {
        self.try_take_then(metadata, |src| T::into_owned(src.trust()).into())
    }
}

pub trait Get : TryGet {
    #[track_caller]
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>;

    #[track_caller]
    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R;

    #[track_caller]
    unsafe fn take<T: ?Sized>(self, metadata: T::Metadata) -> MaybeValid<T::Owned>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>
    {
        self.take_then(metadata, |src| T::into_owned(src.trust()).into())
    }
}

pub trait TryGetMut : TryGet {
    unsafe fn try_get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Error>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>;
}

pub trait GetMut : Get + TryGetMut {
    #[track_caller]
    unsafe fn get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> MaybeValid<&mut T>
        where T: LoadRef,
              Self::Zone: AsZone<T::Zone>;
}

