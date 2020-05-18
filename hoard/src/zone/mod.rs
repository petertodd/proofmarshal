//! Zones where data can be stored.

use std::fmt;

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

pub trait Get : Zone {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a Self::Ptr, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<Self>;

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: Self::Ptr, metadata: T::Metadata) -> T::Owned
        where T: Load<Self>;
}

pub trait GetMut : Get {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut Self::Ptr, metadata: T::Metadata) -> &'a mut T
        where T: Load<Self>;
}

pub trait Alloc : Zone {
}


/*
use core::any::{Any, type_name};
use core::borrow::Borrow;
use core::mem::ManuallyDrop;
use core::fmt;
use core::ops;

use owned::{Take, IntoOwned};

use crate::marshal::blob::ValidateBlob;
use crate::marshal::load::Load;
use crate::pointee::Pointee;

pub mod refs;
use self::refs::*;

pub mod fatptr;
pub use self::fatptr::FatPtr;

pub mod validptr;
pub use self::validptr::ValidPtr;

pub mod ownedptr;
pub use self::ownedptr::OwnedPtr;

pub mod never;
pub mod missing;
pub use self::missing::Missing;

pub trait Zone : Sized + fmt::Debug {
    type Ptr : Copy + Eq + Ord + fmt::Debug + core::hash::Hash + Send + Sync;
    type Persist : 'static + Zone<Ptr=Self::PersistPtr, PersistPtr=Self::PersistPtr>;
    type PersistPtr : 'static + crate::marshal::Primitive + ValidateBlob
                      + Copy + Eq + Ord + fmt::Debug + core::hash::Hash + Send + Sync;

    type Error : std::error::Error;

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> OwnedPtr<T, Self>
        where Self: Default
    {
        unimplemented!("{} implements Default", type_name::<Self>())
    }

    fn duplicate(&self) -> Self;

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self>
        where Self: Clone
    {
        unsafe {
            let new_ptr = FatPtr {
                raw: Self::clone_ptr_unchecked::<T>(&ptr.raw),
                metadata: (),
            };
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(new_ptr))
        }
    }

    unsafe fn clone_ptr_unchecked<T: Clone>(ptr: &Self::Ptr) -> Self::Ptr
        where Self: Clone;

    fn fmt_debug_valid_ptr<T: ?Sized + Pointee, P>(ptr: &P, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug,
              P: Borrow<ValidPtr<T, Self>>,
    {
        match Self::try_get_dirty(ptr.borrow()) {
            Ok(r) => r.fmt(f),
            Err(fatptr) => fmt::Debug::fmt(&fatptr, f),
        }
    }


    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>>;

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(ptr: &Self::Ptr, metadata: T::Metadata)
        -> Result<&T, FatPtr<T, Self::Persist>>
    {
        let ptr = FatPtr { raw: *ptr, metadata };
        let ptr = ValidPtr::<T, Self>::new_unchecked(ptr);

        match Self::try_get_dirty(&ptr) {
            Ok(r) => Ok(&*(r as *const T)),
            Err(fatptr) => Err(fatptr),
        }
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R;

    fn try_take_dirty<T: ?Sized + Pointee + IntoOwned>(owned: OwnedPtr<T, Self>) -> Result<T::Owned, FatPtr<T, Self::Persist>> {
        Self::try_take_dirty_unsized(owned, |src| {
            match src {
                Err(fatptr) => Err(fatptr),
                Ok(unsized_value) => unsafe { Ok(T::into_owned_unchecked(unsized_value)) },
            }
        })
    }
}

pub trait Alloc : Zone {
    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> OwnedPtr<T, Self>;
}

pub trait TryGet : Zone {
    fn try_get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>)
        -> Result<Ref<'a, T, Self>, Self::Error>
    {
        unsafe { self.try_get_unchecked::<T>(&ptr.raw, ptr.metadata) }
    }

    unsafe fn try_get_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a Self::Ptr, metadata: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>;

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        let ptr = ptr.into_inner().into_inner();
        unsafe { self.try_take_unchecked::<T>(ptr.raw, ptr.metadata) }
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: Self::Ptr, metadata: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>;
}

pub trait TryGetMut : TryGet {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>)
        -> Result<RefMut<'a, T, Self>, Self::Error>
    {
        let metadata = ptr.metadata;
        unsafe { self.try_get_mut_unchecked(ptr.raw_mut(), metadata) }
    }

    unsafe fn try_get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut Self::Ptr, metadata: T::Metadata)
        -> Result<RefMut<'a, T, Self>, Self::Error>;
}

pub trait Get : Zone {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self> {
        unsafe { self.get_unchecked(&ptr.raw, ptr.metadata) }
    }

    unsafe fn get_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a Self::Ptr, metadata: T::Metadata) -> Ref<'a, T, Self>;

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self> {
        let ptr = ptr.into_inner().into_inner();
        unsafe { self.take_unchecked::<T>(ptr.raw, ptr.metadata) }
    }

    unsafe fn take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: Self::Ptr, metadata: T::Metadata) -> Own<T::Owned, Self>;
}

impl<Z: TryGet> Get for Z
where Z::Error: Into<!>
{
    unsafe fn get_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a Z::Ptr, metadata: T::Metadata) -> Ref<'a, T, Self> {
        match self.try_get_unchecked::<T>(ptr, metadata) {
            Ok(r) => r,
            Err(e) => match Into::<!>::into(e) {},
        }
    }

    unsafe fn take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: Z::Ptr, metadata: T::Metadata) -> Own<T::Owned, Self> {
        match self.try_take_unchecked::<T>(ptr, metadata) {
            Ok(r) => r,
            Err(e) => match Into::<!>::into(e) {},
        }
    }
}

pub trait GetMut : Get {
    fn get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>) -> RefMut<'a, T, Self> {
        let metadata = ptr.metadata;
        unsafe { self.get_mut_unchecked::<T>(ptr.raw_mut(), metadata) }
    }

    unsafe fn get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut Self::Ptr, metadata: T::Metadata)
        -> RefMut<'a, T, Self>;
}

impl<Z: TryGetMut> GetMut for Z
where Z::Error: Into<!>
{
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut Z::Ptr, metadata: T::Metadata)
        -> RefMut<'a, T, Self>
    {
        match self.try_get_mut_unchecked::<T>(ptr, metadata) {
            Ok(r) => r,
            Err(e) => match Into::<!>::into(e) {},
        }
    }
}

/*
/// Generic pointer.
pub trait Ptr : Sized + NonZero + fmt::Debug {
    /// The persistent version of this pointer, if applicable.
    ///
    /// # Safety
    ///
    /// If this is an inhabited type, it must have the same layout as `Self`. Ideally this would be
    /// expressed as a `Cast<Self>` bound on `Persist`. But this is awkward to implement as
    /// `Persist` has a `Copy` bound that `Self` does not.
    type Persist : Primitive + Copy + fmt::Debug;
    type Zone : Zone<Self> + Copy + Eq + Ord + core::hash::Hash + fmt::Debug;

    fn zone() -> Self::Zone where Self: Default;

    /*
    type Allocator : Alloc<Ptr=Self> + Eq + Ord + core::hash::Hash + fmt::Debug;
    */
}

//pub trait PersistPtr<P> : Ptr + Cast<P> {
//}

/// Mutable `Ptr`.
pub trait PtrMut : Ptr<Zone : ZoneMut<Self>> {
}

pub trait Zone<P: Ptr> {
    fn get<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T, P>) -> Ref<'a, T, P>;

    //fn take<T: ?Sized + Load<P>>(&self, ptr: OwnedPtr<T, P>) -> Own<T::Owned, P>;
}

pub trait ZoneMut<P: Ptr> : Zone<P> {
}


/*
impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, A::Ptr> {
        (**self).alloc(src)
    }

    fn zone(&self) -> <A::Ptr as Ptr>::Zone {
        (**self).zone()
    }
}
*/
*/
*/
