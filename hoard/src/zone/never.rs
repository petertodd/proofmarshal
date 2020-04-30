//! Uninhabited zones and pointers.

use super::*;

impl Zone for ! {
    type Ptr = !;
    type Persist = !;
    type PersistPtr = !;

    type Error = !;

    fn duplicate(&self) -> Self {
        match *self {}
    }

    unsafe fn clone_ptr_unchecked<T: Clone>(ptr: &!) -> ! {
        match *ptr {}
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        match ptr.raw {}
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        _: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        match owned.raw {}
    }
}

impl TryGet for ! {
    unsafe fn try_get_unchecked<'a, T: ?Sized + Load<Self>>(&self, _: &'a !, _: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>
    {
        match *self {}
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, _: !, _: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        match *self {}
    }
}

impl TryGetMut for ! {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, _: &'a mut !, _: T::Metadata)
        -> Result<RefMut<'a, T, Self>, Self::Error>
    {
        match *self {}
    }
}

impl Alloc for ! {
    fn alloc<T: ?Sized + Pointee>(&self, _: impl Take<T>) -> OwnedPtr<T, Self> {
        match *self {}
    }
}
