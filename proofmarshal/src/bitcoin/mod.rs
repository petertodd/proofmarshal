use core::mem;
use core::fmt;

use persist::{Persist, Le};
use persist_derive::Persist;

pub mod state;

#[repr(transparent)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct Txid(Hash);

#[repr(C)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct OutPoint {
    txid: Txid,
    vout: Le<u32>,
}

/// A block hash.
#[repr(C)]
#[derive(Persist,Default,Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHash(Hash);

#[repr(C)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq)]
pub struct MerkleRoot(Hash);

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

impl BlockHeader {
    /// Calculate the block hash.
    pub fn block_hash(&self) -> BlockHash {
        let mut canonical = [0u8; mem::size_of::<Self>()];
        self.write_canonical_bytes(&mut canonical[..]);

        BlockHash(Hash::hash_bytes(&canonical))
    }
}

pub struct Transaction {
}


/// Bitcoin hash
#[repr(transparent)]
#[derive(Persist,Debug,Clone,Copy,PartialEq,Eq,Default)]
pub struct Hash([u8;32]);

impl Hash {
    pub fn hash_bytes(buf: &[u8]) -> Self {
        use sha2::digest::Digest;
        let d = sha2::Sha256::digest(buf);
        let d = sha2::Sha256::digest(&d[..]);

        let mut r = [0u8;32];
        r.copy_from_slice(&d[..]);
        Hash(r)
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.0[..].iter().rev() {
            write!(f,"{:x}", b)?;
        }
        Ok(())
    }
}
