use super::*;

/// A *value* that can be loaded from an arena.
pub trait Load<A: Arena> : Pointee {
    type Error: 'static + fmt::Debug;

    fn load_from_blob<'p>(arena: &A, offset: &'p Ptr<Self, A::Offset>) -> Result<Ref<'p, Self>, Self::Error>
        where A: LoadBlob;
}

/// A *value* that can be stored in an arena.
pub trait Store<A: Arena> : Pointee {
    fn store_to_blob(owned: Own<T,A>) -> (A::Offset, A)
        where A: StoreBlob;
}


/// An *arena* that can load blobs.
pub trait LoadBlob : Arena {
    type Offset;

    /// Same as `try_load_ptr()` but with the additional restriction that the type be possible to
    /// load in place.
    fn try_load_offset<'p,T>(&self, offset: &'p Ptr<T, Self::Offset>) -> Result<&'p Ptr<T, Self::Ptr>, Self::Error>
        where T: ?Sized + Load<Self>;
}

/// An *arena* that can store blobs.
pub trait StoreBlob : Arena {
    type Offset;

    fn store_ptr<T>(&mut self, ptr: Ptr<T, Self::Ptr>) -> Self::Offset
        where T: Store<Self>;

    fn store_bytes(&mut self, buf: &[u8]) -> Self::Offset;
}
