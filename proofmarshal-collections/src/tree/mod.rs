use std::marker::PhantomData;
use std::fmt;

use hoard::prelude::*;
use proofmarshal_derive::Prune;

use crate::fact::{Fact, maybe::Maybe};
use crate::merklesum::MerkleSum;

/// Unbalanced merkle tree.
pub struct Tree<T, S = ()> {
    marker: PhantomData<fn(&T) -> S>,
    pub sum: S,
}

impl<T, S: fmt::Debug> fmt::Debug for Tree<T,S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Tree")
            .field("sum", &self.sum)
            .finish()
    }
}

impl<Z: Zone, T: Fact<Z>, S: MerkleSum<T>> Fact<Z> for Tree<T, S> {
    type Evidence = Node<T, Z, S>;

    fn from_evidence(node: &Self::Evidence) -> Self {
        match node {
            Node::Leaf(item) => Self {
                marker: PhantomData,
                sum: S::from_item(item),
            },
            Node::Inner { left, right } => Self {
                marker: PhantomData,
                sum: S::saturating_sum(&left.sum, &right.sum),
            }
        }
    }
}

#[derive(Debug, Prune)]
#[repr(C, u8)]
pub enum Node<T: Fact<Z>, Z: Zone, S: MerkleSum<T> = ()> {
    Leaf(Maybe<T, Z>),
    Inner {
        left: Maybe<Tree<T, S>, Z>,
        right: Maybe<Tree<T, S>, Z>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_try_join() {
    }
}
