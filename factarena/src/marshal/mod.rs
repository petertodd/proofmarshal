use core::borrow::Borrow;
use core::any::type_name;

use crate::pointee::{Pointee, emplace::Emplace};

pub mod blob;
use self::blob::BlobRef;

/// Pointer to a value in an arena.
pub struct Ptr<T: ?Sized + Pointee, P> {
    marker: PhantomData<*const T>,
    raw: P,
    metadata: T::Metadata,
}

/// Combination of an owned value and an arena.
pub struct Own<T, A: Arena> {
    ptr: Ptr<T, A::Ptr>,
    arena: A,
}

impl<T,A> Own<T,A> {
    pub fn new_in(value: T, arena: A)
        where A: Alloc
    {
        unimplemented!()
    }
}

pub trait Alloc {
    fn alloc(&mut self, value: T) -> Ptr<T,Self::PTr>;
}



//pub mod primitive;

pub trait Arena {
    type Ptr;
    type Error;
}

pub trait Type : Pointee {
    type Owned : Borrow<Self>;

    /// The persistent form of this type, if supported.
    type Persist : ?Sized;

    unsafe fn from_persist_ref(persist: &Self::Persist) -> &Self {
        unimplemented!()
    }

    /*
    fn from_persist(persist: Self::Persist) -> Self
        where Self: Sized,
    {
        unimplemented!()
    }
    */
}

pub unsafe trait Persist {}

pub trait Load<A: Arena> : Type {
    type Error;

    /*
    /// Validate persistently stored data.
    fn validate_persist<'a>(unver: BlobRef<'a, Self::Persist>, ptr_validator: &mut impl ValidatePtr<A>)
        -> Result<&'a Self::Persist, Self::Error>
    {
        let (_, _) = (unver, ptr_validator);
        panic!("{} doesn't implement persistence", type_name::<Self>())
    }
    */
}


pub trait ValidatePtr<A: Arena> {
    fn validate_ptr<T: ?Sized + Load<A>>(&mut self, ptr: &A::Ptr, metadata: T::Metadata) -> Result<(), A::Error>;
}

/*
/// A *type* that can be stored in an arena `A` behind a pointer.
pub trait Store<A> : Load {
    fn 

    /*

    /// Store persistently.
    fn store_blob<S: StorePtr<A>>(&self, dumper: S) -> Result<S::Ptr, S::Error> {
        unimplemented!()
    }
    */
}

/*
/// A *value* that can be stored persistently.
pub trait Value<A> : Sized {
    type Persist : Sized + Persist;

    fn encode_persist<E: StorePtr<A>>(&self, encoder: E) -> Result<Self::Persist, E::Error>;
}

pub unsafe trait StorePtr<A> : Sized {
    type Ptr;
    type Error;

    //fn alloc_blob<T: Persist + Store<A>>(self, persist: &T) -> Result<(Self, Self::Ptr), Self::Error>;
}

pub unsafe trait Persist {
    /// Write a canonical representation of this persistent value to `dst`.
    fn write_canonical(&self, dst: &mut [u8]);
}
*/*/
