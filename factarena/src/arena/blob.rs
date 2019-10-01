/// Persistent arenas that store blobs of bytes.

use super::*;

pub trait BlobArena : Arena<Ptr=<Self as BlobArena>::BlobPtr> {
    type BlobPtr : fmt::Debug + Persist;
}

pub trait AllocBlob<A: Arena> {
    fn alloc_blob<T: Persist<A>>(&mut self, value: &T) -> Own<T,A>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
