use std::io;
use std::any::type_name;

mod digest;
pub use self::digest::Digest;

use crate::fact::{Fact, Valid};

pub trait Commit {
    type Committed;

    fn commit(&self) -> Digest<Self::Committed>;
}

/// Digests commit to themselves
impl<T: 'static + ?Sized> Commit for Digest<T> {
    type Committed = Self;
    fn commit(&self) -> Digest<Self::Committed> {
        unimplemented!()
    }
}

/// References commit to the referenced type.
impl<T: ?Sized + Commit> Commit for &'_ T {
    type Committed = T::Committed;

    fn commit(&self) -> Digest<T::Committed> {
        (**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for &'_ mut T {
    type Committed = T::Committed;

    fn commit(&self) -> Digest<T::Committed> {
        (**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for Box<T> {
    type Committed = T::Committed;
    fn commit(&self) -> Digest<Self::Committed> {
        (**self).commit()
    }
}

//#[derive(Debug)]
pub struct Error<T: ?Sized> {
    actual: Digest<T>,
    expected: Digest<T>,
}

impl<T> Fact for Digest<T> {
    type Evidence = T;
    type Context = ();
    type Error = Error<T>;

    fn validate_in<'cx>(&self, evidence: &Self::Evidence, _: &'cx Self::Context)
        -> Result<&Valid<'cx, Self>, Self::Error>
    {
        unimplemented!()
    }
}
