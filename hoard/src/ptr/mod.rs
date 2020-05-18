use std::fmt;
use std::cmp;

use owned::Take;

use crate::pointee::Pointee;
use crate::load::Load;

pub mod fatptr;
pub use self::fatptr::FatPtr;

pub mod ownedptr;
pub use self::ownedptr::OwnedPtr;

mod never;

pub trait Ptr : Sized + fmt::Debug {
    type Persist : 'static + fmt::Debug;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata);

    fn duplicate(&self) -> Self;

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self
        where Self: Clone;

    /*
    fn alloc_raw<T: ?Sized + Pointee>(src: impl Take<T>) -> (Self, T::Metadata)
        where Self: Default
    {
        unimplemented!()
    }

    */

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist>;
}

/*
pub trait Get<P> {
    unsafe fn get_unchecked<'p, T: ?Sized + Load<Self, P>>(&self, ptr: &'p P, metadata: T::Metadata) -> &'p T;
}
*/

/*
pub trait GetMut<P: Ptr> : Get<P> {
    fn get_mut<'p, T: ?Sized + Load>(&self, ptr: &'p mut Bag<T, P>) -> &'p mut T {
        let metadata = ptr.metadata;
        unsafe {
            self.get_mut_unchecked(ptr.raw_mut(), metadata)
        }
    }

    unsafe fn get_mut_unchecked<'p, T: ?Sized + Load>(&self, ptr: &'p mut P, metadata: T::Metadata) -> &'p mut T;
}

pub trait Alloc {
    type Ptr : Ptr;
    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> Bag<T, Self::Ptr>;
}
*/

/*
pub trait AsPtr<Q> {
    fn as_ptr(&self) -> &Q;
}
*/
