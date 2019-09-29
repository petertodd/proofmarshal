//! Double-spend prevention via single-use-seals.
//!
//! A single-use-seal ("seal" for short) is a globally unique object that starts off *open*, and
//! can be *closed over* a message exactly once, producing a witness attesting to the fact that the
//! seal was closed. Think of it like a pubkey, but with the "magical" property that the pubkey can
//! only sign once. Or like the uniquely numbered zip-tie like seals often used for things like
//! shipping containers to ensure that the contents aren't tempered with.
//!
//! Seals are the core anti-double-spend primitive in Proofmarshal.

use persist_derive::Persist;

use crate::digest::{Digest, Sha256Digest};
use crate::bitcoin::OutPoint;

pub struct BitcoinSeal<T> {
    outpoint: OutPoint,
    nonce: [u8;16],
}

/*
pub enum MultiSeal<T> {
    Bitcoin(BitcoinSeal<T>),
}


impl<T> Seal for BitcoinSeal<T> {
    type Target = T;
}



pub struct Sealed<T> {
    value: T,
}
*/


/*
    Digest(Digest<T>),
    Bitcoin(Digest<BitcoinOutPoint>),
}

#[repr(C)]
#[derive(Persist,Clone,Copy,PartialEq,Eq)]
pub struct BitcoinOutPointSeal {
    outpoint: OutPoint,
    nonce: [u8;16],
}
*/


/*
/// The fact that seal `S` has been closed over `T`.
pub struct Closed<S: Seal<T>, T> {
    marker: PhantomData<fn() -> S>,
    value: T,
}

impl<S,T> Closed<S,T> {
    /// Implicitly trust this fact to be true.
    pub fn trust(value: T) -> Self {
        Closed { marker: PhantomData, value }
    }
}
*/

/*
pub trait Seal<T> {
}

impl<T: 'static,U> Seal<U> for Digest<T>
where U: Commit<Commitment=Digest<T>>,
{
    type Error = !;
    type Witness = ();
}

/// A broken seal that is impossible to close.
pub struct Broken;
*/

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
