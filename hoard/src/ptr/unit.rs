use super::*;

impl Ptr for () {
    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
        todo!()
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, _: T::Metadata) {
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
    }

    unsafe fn fmt_debug_valid_ptr<T: ?Sized + Pointee>(&self, _: T::Metadata, f: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
}
