//! Double-spend prevention via single-use-seals.
//!
//! A single-use-seal ("seal" for short) is a globally unique object that starts off *open*, and
//! can be *closed over* a message exactly once, producing a witness attesting to the fact that the
//! seal was closed. Think of it like a pubkey, but with the magical property that the pubkey can
//! only sign once. Or like the uniquely numbered zip-tie like seals often used for things like
//! shipping containers to ensure that the contents aren't tempered with.
//!
//! Seals are the core anti-double-spend primitive in Proofmarshal.

use std::marker::PhantomData;

use hoard::prelude::*;
use hoard::zone::Missing;

use proofmarshal_core::commit::{Digest, Verbatim, WriteVerbatim};
use proofmarshal_core::fact::{Fact, Maybe};

pub trait SingleUseSeal {
    type Error;
    type Witness;

    fn validate_witness(&self, witness: &Self::Witness, digest: Digest) -> Result<(), Self::Error>;
}

pub struct Seal<T, S> {
    marker: PhantomData<fn(&()) -> &T>,
    raw: S,
}

impl<T, S: Verbatim> Verbatim for Seal<T, S> {
    const LEN: usize = S::LEN;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        self.raw.encode_verbatim(dst)
    }
}

pub struct Sealed<T: Fact<Z>, W: Fact<Z>, Z: Zone = Missing> {
    witness: Maybe<W, Z>,
    value: Maybe<T, Z>,
}

impl<T: Fact<Z>, W: Fact<Z>, Z: Zone> Sealed<T,W,Z> {
    pub fn get_value<S>(&self, seal: S) -> Result<&Maybe<T, Z>, S::Error>
        where S: SingleUseSeal<Witness = W>
    {
        todo!()
    }
}

impl<Z: Zone, T: Fact + Fact<Z>, W: Fact + Fact<Z>> Fact<Z> for Sealed<T, W>
where T: Clone,
      W: Clone
{
    type Evidence = Sealed<T,W,Z>;

    fn from_evidence(evidence: &Self::Evidence) -> Self {
        Self {
            witness: Fact::from_evidence(&evidence.witness),
            value: Fact::from_evidence(&evidence.value),
        }
    }
}
