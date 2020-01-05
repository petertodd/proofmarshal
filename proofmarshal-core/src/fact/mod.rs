//! Fact validation.

use hoard::prelude::*;
use hoard::pointee::Pointee;
use hoard::zone::Missing;

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

impl<T, Z> Fact<Z> for Digest<T> {
    type Evidence = !;

    fn from_evidence(evidence: &Self::Evidence) -> Self {
        todo!()
    }
}
