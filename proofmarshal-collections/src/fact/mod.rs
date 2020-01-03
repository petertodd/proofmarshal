//! Fact validation.

use hoard::prelude::*;

use crate::commit::Commit;

pub mod maybe;

/// A fact that can be derived from evidence.
///
/// The derivation **must not fail**.
pub trait Fact<Z> {
    type Evidence; //: Commit;

    fn from_evidence(evidence: &Self::Evidence) -> Self;
}

/// Evidence pruning.
pub trait Prune {
    /// Marks all evidence as pruned.
    fn prune(&mut self);

    /// Discards pruned evidence, keeping only the evidence that has actually been accessed.
    fn fully_prune(&mut self);
}


/*

impl<Z: Zone, T: Fact> Maybe<T, Z> {
    /// Creates a new `Maybe` from the evidence.
    pub fn new(evidence: T::Evidence) -> Self
        where Z: Default,
    {
        Z::alloc(evidence).into()
    }

    /// Creates a new `Maybe` from the evidence.
    pub fn new_in(evidence: T::Evidence, zone: &Z) -> Self
        where Z: Alloc,
    {
        zone.alloc(evidence).into()
    }

    /// Creates a new `Maybe` from the fact.
    pub fn from_fact(fact: T) -> Self {
        Self {
            evidence: State::Missing,
            fact: Lazy::some(fact),
        }
    }

    /*

    /// Gets the fact, trusting that it's valid.
    pub fn trust(&self) -> &T {
        if let Some(r) = self.fact.get() {
            r
        } else {
            let own = match &self.evidence {
                State::Missing => panic!("Evidence and derived fact both missing"),
                State::Avail(own) | State::Pruned(own) => own,
            };

            // FIXME: more thought needed here...
            let evidence = own.debug_get().expect("Evidence missing");

            let fact = T::from_evidence(&evidence);

            // It's possible the set will fail if another thread is co-currently dereferencing this
            // fact. That's ok and can be ignored.
            let _ = self.fact.try_set(fact);

            self.fact.get().expect("Derived fact available after setting it")
        }
    }
    pub fn state(&self) -> &State<T,P> {
        &self.evidence
    }

    /// Returns true if the evidence is available.
    pub fn is_avail(&self) -> bool {
        match &self.evidence {
            State::Missing | State::Pruned(_) => false,
            State::Avail(_) => true,
        }
    }

    /// Get the evidence, if available.
    ///
    /// Evidence has been pruned is considered unavailable.
    pub fn get<'a>(&'a self) -> Option<Cow<'a, T::Evidence>>
    where T::Evidence: Load<P>,
          P: Get,
    {
        match &self.evidence {
            State::Missing | State::Pruned(_) => None,
            State::Avail(r) => Some(r.get()),
        }
    }

    /// Get the evidence, unpruning if necessary.
    pub fn unprune<'a>(&'a mut self) -> Option<Cow<'a, T::Evidence>>
        where T::Evidence: Prune + Load<P>,
              P: GetMut,
    {
        let new_state = match mem::replace(&mut self.evidence, State::Missing) {
            State::Missing => State::Missing,
            State::Avail(own) => State::Avail(own),
            State::Pruned(mut own) => {
                own.get_mut().prune();
                State::Avail(own)
            }
        };
        mem::replace(&mut self.evidence, new_state);

        match &self.evidence {
            State::Missing => None,
            State::Avail(x) => Some(x.get()),
            State::Pruned(_) => unreachable!("Evidence has been unpruned"),
        }
    }

    /// Same as `unprune()`, but provides mutable access to the evidence.
    pub fn unprune_mut(&mut self) -> Option<&mut T::Evidence>
        where T::Evidence: Prune + Load<P>,
              P: GetMut,
    {
        let _ = self.unprune();

        match &mut self.evidence {
            State::Missing => None,
            State::Avail(own) => Some(own.get_mut()),
            State::Pruned(_) => unreachable!("Evidence has been unpruned"),
        }
    }
    */
}

/*
impl<T: Fact<P>, P: Get> Prune for Maybe<T,P>
where T::Evidence: Load<P>,
      P: Get,
{
    fn prune(&mut self) {
        let new_state = match mem::replace(&mut self.evidence, State::Missing) {
            State::Missing => State::Missing,
            State::Pruned(x) => State::Pruned(x),
            State::Avail(x) => State::Pruned(x),
        };
        mem::replace(&mut self.evidence, new_state);
    }

    fn fully_prune(&mut self) {
        if let State::Pruned(_) = &self.evidence {
            // We're about to discard the evidence, so make sure the fact has been already derived.
            let _ = self.trust();
        };

        let new_state = match mem::replace(&mut self.evidence, State::Missing) {
            State::Missing | State::Pruned(_) => State::Missing,
            State::Avail(x) => State::Avail(x),
        };
        mem::replace(&mut self.evidence, new_state);
    }
}

impl<T: Fact<P>, P: Ptr> Commit for Maybe<T,P>
where T: Commit,
{
    type Committed = T::Committed;

    fn encode_commit_verbatim<W: io::Write>(&self, dst: W) -> Result<W, io::Error> {
        self.trust().encode_commit_verbatim(dst)
    }
}

impl<T: Fact<P>, P: Ptr, Q: Ptr> Verbatim<Q> for Maybe<T,P>
where T: Verbatim<Q>,
      T::Evidence: Verbatim<Q>,
{
    type Error = !;

    const LEN: usize = <T as Verbatim<Q>>::LEN;
    const NONZERO_NICHE: bool = <T as Verbatim<Q>>::NONZERO_NICHE;

    fn encode<W: io::Write>(&self, dst: W, ptr_encoder: &mut impl PtrEncode<Q>) -> Result<W, io::Error> {
        self.trust().encode(dst, ptr_encoder)
    }

    fn decode(_src: &[u8], _ptr_decoder: &mut impl PtrDecode<Q>) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl<T: Fact<P>, P: Ptr> Clone for Maybe<T,P>
where T: Clone,
      T::Evidence: Clone,
{
    fn clone(&self) -> Self {
        Self {
            evidence: self.evidence.clone(),
            fact: self.fact.clone(),
        }
    }
}

impl<T: Fact<P>, P: Ptr> fmt::Debug for Maybe<T,P>
where T: fmt::Debug,
      T::Evidence: fmt::Debug,
      P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Maybe")
            .field("evidence", &self.evidence)
            .field("fact", &self.fact)
            .finish()
    }
}


/*

/// Computes a `Fact` lazily from (owned) evidence.
///
/// Implements `Deref` and `DerefMut` with `T::Evidence` as the `Target`. Mutable access
/// automatically invalidates the derived fact, which is then lazily recomputed when needed.
pub struct Cache<T: Fact<P>, P: Ptr = ()> {
    fact: Lazy<T>,
    evidence: Own<T::Evidence, P>,
}

impl<T: Fact<P>, P: Ptr> Cache<T,P> {
    pub fn new(evidence: Own<T::Evidence, P>) -> Self {
        Self {
            evidence,
            fact: Lazy::uninit(),
        }
    }
}

impl<T: Fact<P>, P: Get> Cache<T,P> {
    pub fn fact(&self) -> &T {
        if let Some(r) = self.fact.get() {
            r
        } else {
            let evidence = self.evidence.get();
            let evidence = T::Evidence::cast(&evidence);
            let fact = T::from_evidence(&evidence);

            // It's possible the set will fail if another thread is co-currently dereferencing this
            // fact. That's ok and can be ignored.
            let _ = self.fact.try_set(fact);

            self.fact.get().expect("Derived fact available after setting it")
        }
    }
}

impl<T: Fact<P>, P: Ptr> Deref for Cache<T,P> {
    type Target = Own<T::Evidence, P>;

    fn deref(&self) -> &Self::Target {
        &self.evidence
    }
}

impl<T: Fact<P>, P: Ptr> DerefMut for Cache<T,P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let _ = self.fact.take();
        &mut self.evidence
    }
}

impl<T: Fact<P>, P: Get, Q> verbatim::Verbatim<Q> for Cache<T,P>
where T: verbatim::Verbatim<Q>,
      Own<T::Evidence, P>: verbatim::Verbatim<Q>,
{
    type Error = !;
    const LEN: usize = <Own<T::Evidence, P> as verbatim::Verbatim<Q>>::LEN + T::LEN;
    const NONZERO_NICHE: bool = <Own<T::Evidence, P> as verbatim::Verbatim<Q>>::NONZERO_NICHE || T::NONZERO_NICHE;

    fn encode<W: io::Write>(&self, dst: W, ptr_encoder: &mut impl verbatim::PtrEncode<Q>) -> Result<W, io::Error> {
        let dst = self.fact().encode(dst, ptr_encoder)?;
        self.evidence.encode(dst, ptr_encoder)
    }

    fn decode(_src: &[u8], _ptr_decoder: &mut impl verbatim::PtrDecode<Q>) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}
*/

/*

macro_rules! impl_nop_prune {
    ($($t:ty,)+) => {
        $(
            impl Prune for $t {
                #[inline]
                fn prune(&mut self) {}

                #[inline]
                fn fully_prune(&mut self) {}
            }
        )+
    }
}

impl_nop_prune! {
    (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

impl<T: Prune> Prune for Option<T> {
    #[inline]
    fn prune(&mut self) {
        if let Some(inner) = self {
            inner.prune()
        }
    }

    #[inline]
    fn fully_prune(&mut self) {
        if let Some(inner) = self {
            inner.fully_prune()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ptr::heap::Heap;
    use crate::verbatim;

    use hex_literal::hex;

    #[test]
    fn maybe() {
        let maybe: Maybe<Digest<u8>, Heap> = Maybe::new(0x23);
        assert_eq!(maybe.trust().to_bytes(),
                   hex!("ff23000000000000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn maybe_verbatim() {
        let maybe: Maybe<Digest<u8>, Heap> = Maybe::new(0x23);
        assert_eq!(crate::verbatim::encode(&maybe),
                   &hex!("ff23000000000000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn maybe_prune_simple() {
        let mut maybe: Maybe<Digest<u8>, Heap> = Maybe::new(0x23);

        // Starts off available
        assert!(maybe.is_avail());

        maybe.prune();
        assert!(!maybe.is_avail());

        assert_eq!(*maybe.unprune().unwrap(), 0x23);
        assert!(maybe.is_avail());

        // Fully prune does *not* make the evidence unavailable as it's currently available.
        maybe.fully_prune();
        assert!(maybe.is_avail());

        // But prune followed by fully prune does.
        maybe.prune();
        maybe.fully_prune();
        assert!(!maybe.is_avail());
        assert_eq!(maybe.unprune(), None);

        // Fully prune should have derived the fact prior to discarding the evidence.
        assert_eq!(maybe.trust().to_bytes(),
                   hex!("ff23000000000000000000000000000000000000000000000000000000000000"));
    }
}
*/
*/
*/
