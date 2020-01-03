use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

/// The canonical "missing" pointer.
impl Ptr for () {
    type Error = Missing;
    type Allocator = Missing;

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self> {
        Own::from_raw(())
    }
    unsafe fn dealloc<T>(self) {
    }
    fn allocator() -> Missing {
        Missing
    }
}

impl TryGet for () {
    unsafe fn try_get<'p, T>(&'p self) -> Result<Cow<'p,T>, Self::Error>
        where T: Load<Self>
    {
        Err(Missing)
    }
    unsafe fn try_take<T>(self) -> Result<T, Self::Error>
        where T: Load<Self>
    {
        Err(Missing)
    }
}

impl TryGetMut for () {
    unsafe fn try_get_mut<T>(&mut self) -> Result<&mut T, Self::Error>
        where T: Load<Self>
    {
        Err(Missing)
    }
}

impl Alloc for Missing {
    type Ptr = ();

    fn alloc<T>(&mut self, value: T) -> Own<T, Self::Ptr>
        where T: Store<Self::Ptr>
    {
        let _ = value;
        unsafe { Own::from_raw(()) }
    }
}
