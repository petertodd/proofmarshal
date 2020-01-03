use std::collections::BTreeMap;
use std::sync::RwLock;

use super::*;

use crate::validate::Valid;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHeight {
    height: u32,
    hash: BlockHash,
}

impl BlockHeight {
    pub fn new(height: u32, hash: BlockHash) -> Self {
        Self {
            hash,
            height,
        }
    }
}

/// Something that can give us the current best block height and block hash.
pub trait BestBlockHeight {
    fn bestblock(&self) -> Valid<BlockHeight, &Self>;
}

/// Validates a `BlockHeight` against the chainstate.
pub trait ValidateBlockhash : BestBlockHeight {
    type Error;

    /// Performs the validation.
    ///
    /// If succesful, ties the `Valid` result to the lifetime of the chainstate.
    fn validate_blockheight(&self, height: BlockHeight) -> Result<Valid<BlockHeight, &Self>, Self::Error>;
}

#[derive(Debug)]
pub struct ChainState {
    heights: RwLock<BTreeMap<u32, BlockHash>>,
}

impl Default for ChainState {
    fn default() -> Self {
        let mut heights = BTreeMap::new();

        // add the genesis block
        heights.insert(0, BlockHash::default());

        Self {
            heights: RwLock::new(heights),
        }
    }
}

impl BestBlockHeight for ChainState {
    fn bestblock(&self) -> Valid<BlockHeight, &Self> {
        let heights = self.heights.read().unwrap();

        let (height, hash) = heights.range(..)
                                    .rev()
                                    .next().expect("missing genesis block");

        Valid::from_trusted(BlockHeight::new(*height, *hash))
    }
}

impl ChainState {
    /// Try to extend the chain without invalidating existing block facts.
    pub fn try_extend(&self, block: BlockHeight) -> Result<Valid<BlockHeight, &Self>, BlockHeight> {
        let mut heights = self.heights.write().unwrap();

        let max_height = heights.keys().rev().next().expect("missing genesis block");

        if *max_height < block.height {
            heights.insert(block.height, block.hash);

            Ok(Valid::from_trusted(block))
        } else {
            Err(block)
        }
    }

    /// Do a reorg.
    ///
    /// Invalidates existing block facts due to requiring mutable access to the chainstate.
    pub fn reorg(&mut self, _block: BlockHeight) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let state = ChainState::default();

        state.try_extend(BlockHeight::new(10, Default::default())).unwrap();

        dbg!(state.bestblock());
    }
}
