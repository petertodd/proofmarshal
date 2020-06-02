use super::*;

impl Ptr for () {
    type Persist = ();

    /*
    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> OwnedPtr<T, Self> {
        todo!()
    }
    */

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
    }

    fn duplicate(&self) -> Self {
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        Err(())
    }
}

/*
impl AsPtr<()> for () {
    fn as_ptr(&self) -> &Self {
        self
    }
}
*/
