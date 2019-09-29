use std::io;
use std::any::type_name;

use crate::digest::Digest;
use crate::fact::{Fact, Proof};
use crate::arena::Coerce;

pub trait Commit {
    fn commit(&self) -> Digest;
}

pub type Commitment<T,A=()> = Proof<Digest<T>, A>;

impl<T,A> Fact<A> for Digest<T>
where T: Coerce<A>,
      T::Coerced: Commit,
{
    type Evidence = T::Coerced;

    fn derive(value: &T::Coerced) -> Digest<T> {
        value.commit().cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    #[derive(Debug)]
    struct Block<T,A> {
        value: Proof<T, A>,
        prev: Option<Commitment<Self, A>>,
    }

    impl Commit for Foo {
        fn commit(&self) -> Digest {
            Digest::default()
        }
    }

    impl<A> Coerce<A> for Foo {
        type Coerced = Foo;
    }

    #[test]
    fn test() {
        let foo = Foo {a: 16, b: 32};

        let mut c: Commitment<Foo> = dbg!(Commitment::from_evidence(foo));

        dbg!(c.unprune());
        dbg!(c);
    }
    */
}
