use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::num::NonZeroU8;

use thiserror::Error;

use hoard::pointee::Pointee;
use hoard::prelude::*;

use crate::commit::Commit;
use crate::fact::{Fact, maybe::Maybe};

use super::merklesum::MerkleSum;

pub mod height;
use self::height::*;

pub struct PerfectTree<T, S = ()> {
    marker: PhantomData<fn() -> T>,
    height: Height,
    sum: S,
}

impl<Z: Zone, T: Fact<Z>, S: MerkleSum<T>> Fact<Z> for PerfectTree<T, S> {
    type Evidence = Node<T, Z, S>;

    fn from_evidence(node: &Self::Evidence) -> Self {
        match node {
            Node::Leaf(item) => Self {
                marker: PhantomData,
                height: Height::default(),
                sum: S::from_item(item),
            },
            Node::Inner { height, sum, .. } => Self {
                marker: PhantomData,
                height: Height::from(*height),
                sum: *sum,
            }
        }
    }
}

pub enum Node<T: Fact<Z>, Z: Zone, S: MerkleSum<T> = ()> {
    Leaf(Maybe<T, Z>),
    Inner {
        left: Maybe<PerfectTree<T, S>, Z>,
        right: Maybe<PerfectTree<T, S>, Z>,
        height: NonZeroHeight,
        sum: S,
    },
}

#[derive(Debug, Error)]
pub enum JoinError<S: std::error::Error> {
    #[non_exhaustive]
    #[error("left height and right heights differ")]
    MismatchedHeights,

    #[non_exhaustive]
    #[error("height overflow")]
    HeightOverflow,

    #[non_exhaustive]
    #[error("sum overflow")]
    SumOverflow {
        err: S,
    }
}

impl<Z: Zone, T: Fact<Z>, S: MerkleSum<T>> Node<T, Z, S> {
    pub fn try_join(left: Maybe<PerfectTree<T, S>, Z>, right: Maybe<PerfectTree<T, S>, Z>) -> Result<Self, JoinError<S::Error>> {
        if left.height != right.height {
            Err(JoinError::MismatchedHeights)
        } else {
            Ok(Self::Inner {
                height: left.height.try_increment().ok_or(JoinError::HeightOverflow)?,
                sum: S::try_sum(&left.sum, &right.sum).map_err(|err| JoinError::SumOverflow { err })?,
                left,
                right,
            })
        }
    }
}
