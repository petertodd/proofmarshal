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

    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        match ptr.raw {}
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
    fn try_get<'a, T: ?Sized + Load<Self>>(&self, _: &'a ValidPtr<T, Self>)
        -> Result<Ref<'a, T, Self>, Self::Error>
    {
        match *self {}
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, _: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        match *self {}
    }
}

impl TryGetMut for ! {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, _: &'a mut ValidPtr<T, Self>)
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
