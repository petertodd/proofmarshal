//! Fact validation.

use std::ops;
use std::marker::PhantomData;

use hoard::prelude::*;
use hoard::pointee::Pointee;
use hoard::zone::Missing;

/*
use hoard::marshal::save::Saved;
use crate::commit::{Digest, Commit, Verbatim};

pub mod error;
pub mod maybe;
pub use self::maybe::Maybe;

/// A fact that can be derived from evidence.
///
/// The derivation **must not fail**.
pub trait Fact<Z = Missing> {
    type Evidence : Pointee;

    fn from_evidence(evidence: &Self::Evidence) -> Self;
}

/// Evidence pruning.
pub trait Prune {
    /// Marks all evidence as pruned.
    fn prune(&mut self);

    /// Discards pruned evidence, keeping only the evidence that has actually been accessed.
    fn fully_prune(&mut self);
}

impl<Z: Zone, T: Fact + Fact<Z>> Fact<Z> for Maybe<T, Missing>
where T: Clone
{
    type Evidence = Maybe<T, Z>;

    fn from_evidence(evidence: &Self::Evidence) -> Self {
        Self::from_fact(evidence.trust().clone())
    }
}
*/

pub trait Derive<Z> {
    type Proof : Pointee;

    fn from_proof(proof: &Self::Proof) -> Self;
}

//pub trait Fact<Z> : Derive<Missing> + Derive<Z> {
//}

pub struct Maybe<T, Z: Zone> {
    state: u8,
    digest: [u8;32],
    fact: T,
    evidence: Z::Ptr,
}

/*
#[repr(transparent)]
pub struct Unverified<'ctx, T> {
    marker: PhantomData<&'ctx ()>,
    inner: T,
}

pub trait Valid<'ctx> {
}

impl<'ctx, T> ops::Deref for Unverified<'ctx, T>
where T: Valid<'ctx>
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}
*/
