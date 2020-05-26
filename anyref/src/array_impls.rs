use super::*;

unsafe impl<'a, T: Static, const N: usize> Static for [T; N] {
    type Static = [T::Static; N];
}

unsafe impl<'a, T: Static + AnyRef<'a>, const N: usize> AnyRef<'a> for [T; N] {
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}
