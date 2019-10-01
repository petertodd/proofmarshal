use super::*;

/// Can live behind a pointer
pub trait Type<A: Arena> : Pointee {
    type Error;

    type RefOwned : Borrow<Self>;

    /*
    fn load_blob<'a>(arena: &impl LoadBlob<A>, own: &'a Own<Self, A>) -> Result<Ref<'a,Self,A>, Self::Error>
        where A: BlobArena;
        */

    /*
    fn store_children<'a>(&mut self, arena: &mut impl StoreBlob<A>) -> A::BlobPtr
        where A: BlobArena;
        */

    fn store_blob<'a>(&self, arena: &mut impl AllocBlob<A>) -> Own<Self, A>
        where A: BlobArena;
}

/*
/// Serializable/deserializable
pub trait Value<A: Arena> : Sized + Metadata<Metadata=()> {
    type Error;

    type Primitives : Persist<A>;
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
