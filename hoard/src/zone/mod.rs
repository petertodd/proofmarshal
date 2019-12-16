//! Zones where data can be stored.

use core::any::type_name;
use core::mem::ManuallyDrop;
use core::fmt;

use nonzero::NonZero;
use owned::Take;

use crate::{
    coerce,
    pointee::Pointee,
    refs::*,
    marshal::{*, blob::*},
};


mod fatptr;
pub use self::fatptr::FatPtr;

mod validptr;
pub use self::validptr::ValidPtr;

mod ownedptr;
pub use self::ownedptr::OwnedPtr;

pub mod never;

/// Generic pointer.
pub trait Ptr : Sized + NonZero + Persist + fmt::Debug {
    /// The persistent version of this pointer, if applicable.
    ///
    /// # Safety
    ///
    /// If this is an inhabited type, it must have the same layout as `Self`. Ideally this would be
    /// expressed as a `Cast<Self>` bound on `Persist`. But this is awkward to implement as
    /// `Persist` has a `Copy` bound that `Self` does not.
    type Persist : Ptr + Primitive + coerce::Cast<Self> + Into<Self> + Copy;

    type Zone : Zone<Self> + Copy + Eq + Ord + core::hash::Hash + fmt::Debug;
    type Allocator : Alloc<Ptr=Self> + Eq + Ord + core::hash::Hash + fmt::Debug;

    fn allocator() -> Self::Allocator where Self: Default;

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self>
        where Self: Clone;

    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>);

    fn fmt_debug_own<T: ?Sized + Pointee>(owned: &OwnedPtr<T, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
        where T: fmt::Debug
    {
        f.debug_struct(type_name::<OwnedPtr<T, Self>>())
            .field("raw", &owned.raw)
            .field("metadata", &owned.metadata)
            .finish()
    }


    fn drop_take<T>(owned: OwnedPtr<T, Self>) -> Option<T> {
        let mut r = None;

        Self::drop_take_unsized(owned,
            |src| unsafe {
                r = Some(ManuallyDrop::take(src));
            }
        );

        r
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>));

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist>;
}

pub trait PtrMut : Ptr<Zone : ZoneMut<Self>> {
}

pub trait Zone<P: Ptr> {
    fn get<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T, P>) -> Ref<'a, T, P>;

    fn take<T: ?Sized + Load<P>>(&self, ptr: OwnedPtr<T, P>) -> Own<T::Owned, P>;

    /*
    fn get_ref<'a, T: ?Sized + Load<P>>(&self, ptr: impl Into<Ref<'a, OwnedPtr<T, P>>>) -> Ref<'a, T>
        where P: 'a
    {
        match ptr.into() {
            Ref::Borrowed(ptr) => self.get(ptr),
            Ref::Owned(ptr) => Ref::Owned(self.take(ptr)),
        }
    }
    */
}

pub trait ZoneMut<P: Ptr> : Zone<P> {
    fn get_mut<'a, T: ?Sized + Load<P>>(&self, ptr: &'a mut ValidPtr<T, P>) -> RefMut<'a, T, P>;
}

pub trait Alloc : Sized {
    type Ptr : Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, Self::Ptr>;
    fn zone(&self) -> <Self::Ptr as Ptr>::Zone;
}

impl<A: Alloc> Alloc for &'_ mut A {
    type Ptr = A::Ptr;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, A::Ptr> {
        (**self).alloc(src)
    }

    fn zone(&self) -> <A::Ptr as Ptr>::Zone {
        (**self).zone()
    }
}

