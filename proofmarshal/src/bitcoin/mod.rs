use persist::Le;
use persist_derive::Persist;

pub mod state;

#[repr(transparent)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct Txid([u8;32]);

#[repr(C)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct OutPoint {
    txid: Txid,
    vout: Le<u32>,
}

/// A block hash.
#[repr(C)]
#[derive(Persist,Default,Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHash([u8;32]);

#[repr(C)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct MerkleRoot([u8;32]);

#[repr(C)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHeader {
    version: Le<i32>,
    prevblock: BlockHash,
    merkleroot: MerkleRoot,
    time: Le<u32>,
    nbits: Le<u32>,
    nonce: Le<u32>,
}

pub struct Transaction {
}
