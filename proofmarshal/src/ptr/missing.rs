use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

impl Ptr for () {
    type Error = Missing;
    type Allocator = Missing;

    unsafe fn clone_ptr<T: ?Sized + Type<Self>>(&self) -> Own<T, Self> {
        Own::from_raw(())
    }
    unsafe fn dealloc<T: ?Sized>(self) {
    }
    fn allocator() -> Missing {
        Missing
    }
}

impl TryGet for () {
    unsafe fn try_get<'p, T>(&'p self) -> Result<Ref<'p,T,Self>, Self::Error>
        where T: ?Sized + Type<Self>
    {
        Err(Missing)
    }
    unsafe fn try_take<'p, T>(self) -> Result<T::Owned, Self::Error>
        where T: ?Sized + Type<Self>
    {
        Err(Missing)
    }
}

impl TryGetMut for () {
    unsafe fn try_get_mut<'p, T>(&'p mut self) -> Result<&'p mut T::Owned, Self::Error>
        where T: ?Sized + Type<Self>
    {
        Err(Missing)
    }
}

impl Alloc for Missing {
    type Ptr = ();

    fn alloc<T>(&mut self, value: T::Owned) -> Own<T, Self::Ptr>
        where T: ?Sized + Type<Self::Ptr>
    {
        let _ = value;
        unsafe { Own::from_raw(()) }
    }
}
