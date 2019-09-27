//! Validation

use core::borrow::Borrow;

use crate::commit::Commit;

pub trait Fact {
    type Context;
}

/// Represents a fact that has been proven to be true and the context in which that was proven.
#[derive(Debug)]
pub struct Valid<T, A = ()> {
    fact: T,
    context: A,
}

impl<T,A> Valid<T,A> {
    pub fn trust(fact: T, context: A) -> Self {
        Self { fact, context }
    }

    pub fn drop_context(self) -> Valid<T> {
        Valid { fact: self.fact, context: () }
    }
}

/// A commitment to a valid fact is the same as a commitment to the fact itself.
impl<T: Commit, A> Commit for Valid<T,A> {
    type Commitment = T::Commitment;

    fn commit(&self) -> Self::Commitment {
        self.fact.commit()
    }
}
