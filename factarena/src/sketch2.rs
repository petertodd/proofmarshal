pub trait Arena {
    type Ptr : Dealloc;
    type Locator;

    /// Not supposed to happen
    type DerefError : 'static;

    fn deref_ptr<T: ?Sized + Pointee>(locator: &Self::Locator, ptr: &Self::Ptr<T, Self>) -> Result<&T, Self::DerefError>;
}

pub trait MutArena : Arena {
    /// Not supposed to happen
    type DerefMutError : 'static;

    fn deref_ptr_mut<'p, T: ?Sized + Pointee>(locator: &mut Self::Locator, ptr: &'p mut Self::Ptr<T, Self>)
        -> Result<&'p mut T, Self::DerefMutError>;
}

/// An allocator that can allocate in an arena
pub trait AllocPtr<A>
