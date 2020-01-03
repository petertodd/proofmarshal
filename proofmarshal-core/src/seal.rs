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
use std::num::NonZeroU128;
use std::ops;

use crate::digest::Digest;
use crate::commit::Commit;

/// A typed wrapper around a raw seal.
#[repr(transparent)]
pub struct Seal<T, S> {
    marker: PhantomData<(fn(T) -> T)>,
    raw: S,
}

impl<T, S> From<S> for Seal<T,S> {
    fn from(raw: S) -> Self {
        Self { marker: PhantomData, raw}
    }
}

/// Contextual proof that a seal has been closed.
pub trait Witness<S, Context> {
    type Error;

    /// Validates that the seal has been closed.
    fn validate(&self, context: &mut Context,
                       seal: &S,
                       digest: Digest) -> Result<(), Self::Error>;
}

/// The fact that a seal has been closed over a value.
#[derive(Debug)]
#[repr(C)]
pub struct Sealed<T,S,W=!,X=!> {
    // Invariant over W and X to be conservative.
    marker: PhantomData<(fn(W) -> X,fn(X) -> X)>,
    seal: Digest<S>,
    value: T,
}

/// A `Sealed` value that hasn't been verified.
pub type MaybeSealed<T,S,W> = Sealed<T,S,W>;

impl<T,S,W> MaybeSealed<T,S,W>
{
    /// Creates a new sealed value, without a context.
    ///
    /// This is an untrusted operation as you won't be able to access the value.
    pub fn new(value: T, seal: Digest<S>) -> Self {
        Self { marker: PhantomData, value, seal }
    }
}

impl<T,S,W,X> Sealed<T,S,W,X>
where W: Witness<S,X>
{
    /// Trusts that a value has been sealed without verifying that fact.
    pub fn trust(maybe: &MaybeSealed<T,S,W>) -> &Self {
        // Safe because `Sealed` is #[repr(C)] and W and X are phantoms.
        unsafe { &*(maybe as *const _ as *const _) }
    }
}

/// If the witness is valid for the specified seal and context validation (or trust) must have
/// happened, so we can dereference to the value.
impl<T,S,W,X> ops::Deref for Sealed<T,S,W,X>
where W: Witness<S,X>
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

/// "Fake" seal that simply hashes the sealed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HashSeal {
    nonce: Option<NonZeroU128>,
    digest: Digest,
}

impl HashSeal {
    /// Creates a new `HashSeal` from a value.
    pub fn new<T: Commit>(value: &T) -> Seal<T, Self> {
        Self {
            nonce: None,
            digest: value.commit().cast(),
        }.into()
    }
}

#[derive(Debug)]
pub struct ValidateHashSealError;

impl<X: AsRef<()>> Witness<HashSeal,X> for () {
    type Error = ValidateHashSealError;

    fn validate(&self, _: &mut X, seal: &HashSeal, digest: Digest) -> Result<(), Self::Error> {
        if seal.digest == digest {
            Ok(())
        } else {
            Err(ValidateHashSealError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let v = 0x1234_5678_u32;

        let seal = HashSeal::new(&v);
        let maybe = MaybeSealed::<_,HashSeal,()>::new(v, Digest::default());
    }
}
