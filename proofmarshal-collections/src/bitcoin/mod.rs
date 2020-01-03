use std::mem;
use std::fmt;

pub mod state;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Txid(Hash);

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct OutPoint {
    txid: Txid,
    vout: u32,
}

/// A block hash.
#[derive(Default,Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHash(Hash);

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct MerkleRoot(Hash);

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct BlockHeader {
    version: i32,
    prevblock: BlockHash,
    merkleroot: MerkleRoot,
    time: u32,
    nbits: u32,
    nonce: u32,
}

impl BlockHeader {
    /// Calculate the block hash.
    pub fn block_hash(&self) -> BlockHash {
        unimplemented!()
    }
}

pub struct Transaction {
}


/// Bitcoin hash
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]
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
