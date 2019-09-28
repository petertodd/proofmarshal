use std::collections::BTreeMap;
use std::sync::RwLock;

use super::*;

use crate::maybe::Valid;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHeight {
    height: Le<u32>,
    hash: BlockHash,
}

impl BlockHeight {
    pub fn new(height: impl Into<Le<u32>>, hash: BlockHash) -> Self {
        Self {
            hash,
            height: height.into(),
        }
    }
}

/// Something that can give us the current best block height and block hash.
pub trait BestBlockHeight {
    fn bestblock(&self) -> Valid<BlockHeight, &Self>;
}

pub trait ValidateBlockhash : BestBlockHeight {
    type Error;

    //fn validate_blockheight(&self, height: BlockHeight) -> Result<Valid<BlockHeight, &Self>, Self::Error>;
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

        Valid::trust(BlockHeight::new(*height, *hash), self)
    }
}

impl ChainState {
    /// Try to extend the chain without invalidating existing block facts.
    pub fn try_extend(&self, block: BlockHeight) -> Result<Valid<BlockHeight, &Self>, BlockHeight> {
        let mut heights = self.heights.write().unwrap();

        let max_height = heights.keys().rev().next().expect("missing genesis block");

        if *max_height < block.height.get() {
            heights.insert(block.height.get(), block.hash);

            Ok(Valid::trust(block, self))
        } else {
            Err(block)
        }
    }

    /// Do a reorg.
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
