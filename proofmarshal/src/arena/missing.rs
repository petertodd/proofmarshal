use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

impl Ptr for () {
    type Error = Missing;
    type Allocator = ();

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self> {
        Own::from_raw(())
    }
    unsafe fn dealloc<T>(self) {
    }
    unsafe fn try_get<'p, T: Clone>(&'p self) -> Result<Cow<'p, T>, Self::Error> {
        Err(Missing)
    }
    unsafe fn try_take<'p, T>(self) -> Result<T, Self::Error> {
        Err(Missing)
    }

    fn allocator() -> () {
        ()
    }
}

impl MutPtr for () {
    unsafe fn try_get_mut<'p, T>(&'p mut self) -> Result<&'p mut T, Self::Error> {
        Err(Missing)
    }
}

impl Alloc for () {
    type Ptr = ();

    fn alloc<T>(&mut self, value: T) -> Own<T, Self::Ptr> {
        let _ = value;
        unsafe { Own::from_raw(()) }
    }
}
