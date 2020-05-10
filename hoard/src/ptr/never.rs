use std::marker::PhantomData;

use super::*;

impl Ptr for ! {
    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, _: T::Metadata) {
        match *self {}
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        match *self {}
    }

    unsafe fn fmt_debug_valid_ptr<T: ?Sized + Pointee>(&self, _: T::Metadata, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {}
    }
}
