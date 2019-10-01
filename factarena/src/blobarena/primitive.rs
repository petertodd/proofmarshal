use super::*;

impl<A: Arena> Store<A> for ! {
    fn store_blob(owned: Self::Owned, alloc: &mut impl AllocBlob<A>) -> (A::Offset, ())
        where A: blob::Arena
    {
        match owned {}
    }
}

macro_rules! impl_primitives {
    ($( $t:ty, )*) => {
        $(
            impl<A: Arena> Load<A> for $t {
                type Error = !;
                fn load_blob<'p>(arena: &A, offset: &'p Ptr<Self, A::Offset>) -> Result<Ref<'p, Self>, Self::Error>
                    where A: blob::Arena
                {
                    unimplemented!()
                }
            }

            impl<A: Arena> Store<A> for $t {
                fn store_blob(owned: $t, alloc: &mut impl AllocBlob<A>) -> (A::Offset, ())
                    where A: blob::Arena
                {
                    alloc.alloc_blob(&owned).into_raw()
                }
            }
        )*
    }
}

impl_primitives! {
    (),
    u8,
    i8,
}
