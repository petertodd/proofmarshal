use std::collections::HashSet;

use persist::Le;

pub mod state;

#[repr(transparent)]
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct Txid([u8;32]);

#[repr(C)]
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct OutPoint {
    txid: Txid,
    vout: Le<u32>,
}


/// A block hash.
#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct BlockHash([u8;32]);

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct MerkleRoot([u8;32]);

#[repr(C)]
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct BlockHeader {
    version: Le<i32>,
    prevblock: Option<BlockHash>,
    merkleroot: MerkleRoot,
    time: Le<u32>,
    nbits: Le<u32>,
    nonce: Le<u32>,
}

pub struct Transaction {
}
