//! Fact validation.

use hoard::prelude::*;

use crate::commit::{Commit, Verbatim};

pub mod error;
pub mod maybe;
pub use self::maybe::Maybe;

/// A fact that can be derived from evidence.
///
/// The derivation **must not fail**.
pub trait Fact<Z> : 'static + Verbatim {
    type Evidence : Commit;

    fn from_evidence(evidence: &Self::Evidence) -> Self;
}

/// Evidence pruning.
pub trait Prune {
    /// Marks all evidence as pruned.
    fn prune(&mut self);

    /// Discards pruned evidence, keeping only the evidence that has actually been accessed.
    fn fully_prune(&mut self);
}
