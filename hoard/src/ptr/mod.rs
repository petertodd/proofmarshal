use std::fmt;

use owned::Take;

use crate::pointee::Pointee;
use crate::load::Load;

pub mod fat;
pub use self::fat::Fat;

pub mod bag;
pub use self::bag::Bag;

pub mod never;
mod unit;

pub trait Ptr : Sized {
    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self>
        where Self: Default
    {
        unimplemented!()
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata);

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self
        where Self: Clone;

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Option<&T> {
        None
    }

    unsafe fn fmt_debug_valid_ptr<T: ?Sized + Pointee>(&self, metadata: T::Metadata, f: &mut fmt::Formatter) -> fmt::Result
        where T: fmt::Debug
    {
        todo!()
    }
}

pub trait Get<P: Ptr> {
    fn get<'p, T: ?Sized + Load>(&self, ptr: &'p Bag<T, P>) -> &'p T {
        unsafe {
            self.get_unchecked(&ptr.raw, ptr.metadata)
        }
    }

    unsafe fn get_unchecked<'p, T: ?Sized + Load>(&self, ptr: &'p P, metadata: T::Metadata) -> &'p T;

    unsafe fn take_unchecked<T: Load>(&self, ptr: P, metadata: T::Metadata) -> T;
}

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
