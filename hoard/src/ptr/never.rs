use std::marker::PhantomData;

use super::*;

impl Ptr for ! {
    type Persist = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
        match *self {}
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        match *self {}
    }

    fn duplicate(&self) -> Self {
        match *self {}
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        match *self {}
    }
}
