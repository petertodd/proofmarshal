pub trait Metadata {
    type Metadata : Sized;
}

pub trait Pointee : Metadata {
    type Owned : Sized + Borrow<Self>;
}

/// Typed pointer.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ptr<T: Metadata, R> {
    marker: PhantomData<T>,
    raw: R,
    metadata: T::Metadata,
}

pub trait Arena {
    type Ptr;
}

pub trait Load<A> : Pointee {
    type Error;
    fn load_ref(ptr: &Ptr<T,A>, arena: &A) -> Result<Ref<Self>, Self::Error>;

    fn load(ptr: Ptr<T,A>, arena: &A) -> Result<Self, Self::Error>
        where Self: Sized;
}

pub trait Store<A> {
    fn store(self, arena: &mut A) -> Result<Ptr<T,A>, A::Error>;
}

pub enum Ref<'p, T: Type> {
    Borrowed(&'p T),
    Owned(T::Owned),
}

/// A type whose values can reside in arena `A`
pub trait Value<A> : Sized {
    /// The basic primitives that make up a value of this type.
    type Primitives : 'static + Marshal<A>;

    type Error;
    fn try_from_primitives(primitives: Self::Primitives) -> Result<Self, Self::Error>;
    fn into_primitives(self) -> Result<Self::Primitives, A::Error>;
}
