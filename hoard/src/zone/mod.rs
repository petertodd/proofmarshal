//! Zones where data can be stored.

use core::any::{Any, type_name};
use core::borrow::Borrow;
use core::mem::ManuallyDrop;
use core::fmt;
use core::ops;

use nonzero::NonZero;
use owned::{Take, Owned};

use crate::{
    coerce,
    pointee::Pointee,
    load::{Load, Persist},
    marshal::Primitive,
};

pub mod refs;
use self::refs::*;

mod error;
pub use self::error::{DerefError, PtrError};

pub trait Zone : Sized {
    type Ptr : NonZero + Copy + Eq + Ord + fmt::Debug + core::hash::Hash + Send + Sync;
    type Persist : 'static + Zone<Ptr=Self::PersistPtr, Error=Self::Error>;
    type PersistPtr : 'static + Primitive + NonZero + Copy + Eq + Ord + fmt::Debug + core::hash::Hash + Send + Sync;

    type Allocator : Alloc<Zone=Self>;

    type Error : fmt::Debug;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        unimplemented!("{} implements Default", type_name::<Self>())
    }

    fn duplicate(&self) -> Self;

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self>
        where Self: Clone;

    fn fmt_debug_valid_ptr<T: ?Sized + Pointee, P>(ptr: &P, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug,
              P: Borrow<ValidPtr<T, Self>>,
    {
        let ptr = ptr.borrow();
        f.debug_struct(type_name::<P>())
            .field("raw", &ptr.raw)
            .field("metadata", &ptr.metadata)
            .finish()
    }


    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>>;

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R;

    fn try_take_dirty<T: ?Sized + Pointee + Owned>(owned: OwnedPtr<T, Self>) -> Result<T::Owned, FatPtr<T, Self::Persist>> {
        Self::try_take_dirty_unsized(owned, |src| {
            match src {
                Err(fatptr) => Err(fatptr),
                Ok(unsized_value) => unsafe { Ok(T::to_owned(unsized_value)) },
            }
        })
    }
}

pub trait Alloc : Sized {
    type Zone : Zone;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, Self::Zone>;
}

pub trait TryGet : Zone {
    fn try_get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>)
        -> Result<Ref<'a, T, Self>, DerefError<T, Self>>;

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, DerefError<T, Self>>;
}

pub trait TryGetMut : TryGet {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>)
        -> Result<RefMut<'a, T, Self>, DerefError<T, Self>>;
}

pub trait Get : Zone {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self>;
    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self>;
}

impl<Z: Get<Error=!>> TryGet for Z {
    fn try_get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>)
        -> Result<Ref<'a, T, Self>, DerefError<T, Self>>
    {
        Ok(self.get(ptr))
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, DerefError<T, Self>>
    {
        Ok(self.take(ptr))
    }
}

pub trait GetMut : Get {
    fn get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>) -> RefMut<'a, T, Self>;
}

impl<Z: GetMut<Error=!>> TryGetMut for Z {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>)
        -> Result<RefMut<'a, T, Self>, DerefError<T, Self>>
    {
        Ok(self.get_mut(ptr))
    }
}


pub mod fatptr;
pub use self::fatptr::FatPtr;

pub mod validptr;
pub use self::validptr::ValidPtr;

pub mod ownedptr;
pub use self::ownedptr::OwnedPtr;

pub mod never;

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
