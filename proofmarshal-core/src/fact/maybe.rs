//! Fact validation.

use std::cell::UnsafeCell;
use std::fmt;
use std::mem::MaybeUninit;
use std::num::NonZeroU8;
use std::ops;
use std::ptr;
use std::sync::atomic::{AtomicU8, Ordering, spin_loop_hint};

use super::{*, error::UnpruneError};

use crate::commit::{Commit, Digest, Verbatim, WriteVerbatim};

/// A fact that may or may not be true.
///
/// The fact itself is always available, and can be accessed with the `trust()` method. The
/// evidence (potentially) proving the fact to be true may or may not be available.
#[repr(C)]
pub struct Maybe<T: Fact<Z>, Z: Zone = !> {
    state: UnsafeCell<NonZeroU8>,
    digest: UnsafeCell<MaybeUninit<Digest>>,
    fact: UnsafeCell<MaybeUninit<T>>,
    evidence: MaybeUninit<OwnedPtr<T::Evidence, Z>>,
}

impl<Z: Zone, T: Fact<Z>> Maybe<T, Z> {
    /// Gets access to the proven fact, trusting that it's true.
    pub fn trust(&self) -> &T {
        todo!()
    }

    /// Tries to get access to the evidence.
    pub fn try_unprune(&self, zone: &Z) -> Result<&T::Evidence, Z::Error> {
        todo!()
    }

    /// Tries to get mutable access to the evidence.
    pub fn try_unprune_mut(&mut self, zone: &Z) -> Result<&mut T::Evidence, Z::Error> {
        todo!()
    }
}

impl<T: Fact<Z>, Z: Zone> Verbatim for Maybe<T, Z>
where T: Verbatim
{
    const LEN: usize = T::LEN + <Digest as Verbatim>::LEN;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        dst.write(Self::get_digest(self))?
           .write(Self::get_fact(self))?
           .finish()
    }
}

impl<Z: Zone, T: Fact<Z>> fmt::Debug for Maybe<T, Z>
where T: fmt::Debug,
{
    default fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let state = self.load_flags(Ordering::Relaxed);
        f.debug_struct("Maybe")
            .field("state", &state)
            .field("digest", &Maybe::try_get_digest(self))
            .field("fact", &Maybe::try_get_fact(self))
            .field("evidence", &"<>")
            .finish()
    }
}

impl<Z: Zone, T: Fact<Z>> fmt::Debug for Maybe<T, Z>
where T: fmt::Debug,
      T::Evidence: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let state = self.load_flags(Ordering::Relaxed);
        f.debug_struct("Maybe")
            .field("state", &state)
            .field("digest", &Maybe::try_get_digest(self))
            .field("fact", &Maybe::try_get_fact(self))
            .field("evidence", &Maybe::try_get_evidence(self))
            .finish()
    }
}

bitflags::bitflags! {
    pub struct Flags: u8 {
        const VALID        = 0b000001;
        const PRUNED       = 0b000010;

        const PRUNABLE     = 0b000100;
        const DIRTY_FACT   = 0b001000;
        const DIRTY_DIGEST = 0b010000;
        const LOCKED       = 0b100000;

        const VOLATILE = Self::PRUNABLE.bits | Self::DIRTY_FACT.bits | Self::DIRTY_DIGEST.bits | Self::LOCKED.bits;
    }
}

impl From<Flags> for NonZeroU8 {
    fn from(flags: Flags) -> Self {
        NonZeroU8::new((flags | Flags::VALID).bits).unwrap()
    }
}

impl From<Flags> for UnsafeCell<NonZeroU8> {
    fn from(flags: Flags) -> Self {
        UnsafeCell::new(flags.into())
    }
}

impl<Z: Zone, T: Fact<Z>> Maybe<T, Z> {
    unsafe fn state_atomic(&self) -> &AtomicU8 {
        &*(&self.state as *const _ as *const _)
    }

    fn load_flags(&self, ordering: Ordering) -> Flags {
        unsafe {
            let n: u8 = self.state_atomic().load(ordering);
            Flags::from_bits(n).expect("flags to be valid")
        }
    }

    fn spinwait_flags(&self) -> Flags {
        loop {
            let flags = self.load_flags(Ordering::Acquire);

            if !flags.contains(Flags::LOCKED) {
                break flags
            } else {
                spin_loop_hint();
            }
        }
    }

    pub fn try_get_digest(this: &Self) -> Option<&Digest> {
        if this.spinwait_flags().contains(Flags::DIRTY_DIGEST) {
            None
        } else {
            unsafe { this.digest.get().cast::<Digest>().as_ref() }
        }
    }

    pub fn get_digest(this: &Self) -> &Digest {
        todo!()
    }

    pub fn try_get_fact(this: &Self) -> Option<&T> {
        if this.spinwait_flags().contains(Flags::DIRTY_FACT) {
            None
        } else {
            unsafe { this.fact.get().cast::<T>().as_ref() }
        }
    }

    pub fn get_fact(this: &Self) -> &T {
        todo!()
    }

    pub fn try_get_evidence(this: &Self) -> Option<&OwnedPtr<T::Evidence, Z>> {
        let flags = this.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::PRUNED) {
            None
        } else {
            Some(unsafe { this.evidence.as_ptr().as_ref().unwrap() })
        }
    }

    pub fn new_in(evidence: T::Evidence, zone: &Z) -> Self
        where Z: Alloc
    {
        let evidence = zone.alloc(evidence);
        Z::try_get_dirty(&evidence).expect("alloc should return dirty");

        Self {
            state: (Flags::VALID | Flags::DIRTY_FACT | Flags::DIRTY_DIGEST).into(),
            fact: MaybeUninit::uninit().into(),
            digest: MaybeUninit::uninit().into(),
            evidence: MaybeUninit::new(evidence),
        }
    }

    /// Creates a new `Maybe` from the fact and digest.
    pub fn from_fact(fact: T, digest: Digest) -> Self {
        Self {
            state: Flags::PRUNED.into(),
            fact: MaybeUninit::new(fact).into(),
            digest: MaybeUninit::new(digest).into(),
            evidence: MaybeUninit::uninit(),
        }
    }
}

impl<Z: Zone, T: Fact<Z>> ops::Deref for Maybe<T, Z> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        loop {
            let flags = self.spinwait_flags();
            if !flags.contains(Flags::DIRTY_FACT) {
                break unsafe { self.fact.get().cast::<T>().as_ref().unwrap() }
            } else {
                assert!(!flags.contains(Flags::PRUNED));
                // SAFETY: not pruned, so the evidence should be there
                let evidence_ptr = unsafe { self.evidence.as_ptr().as_ref().unwrap() };

                // create the fact
                let evidence = Z::try_get_dirty(evidence_ptr).expect("evidence should be available");
                let fact = T::from_evidence(evidence);

                // Try to lock the state
                let prev_flags = unsafe {
                    self.state_atomic().compare_and_swap(flags.bits, (flags | Flags::LOCKED).bits, Ordering::Acquire)
                };

                if prev_flags == flags.bits {
                    // We've succesfully locked the state with no interference from others, so we have
                    // exclusive access.
                    unsafe {
                        self.fact.get().cast::<T>().write(fact);

                        let new_flags = flags & !(Flags::LOCKED | Flags::DIRTY_FACT);
                        self.state_atomic().store(new_flags.bits, Ordering::Release);

                        break self.fact.get().cast::<T>().as_ref().unwrap()
                    }
                }
                // didn't lock the state succesfully, so continue to loop
            }
        }
    }
}

impl<Z: Zone, T: Fact<Z>> Drop for Maybe<T, Z> {
    fn drop(&mut self) {
        let flags = self.load_flags(Ordering::Relaxed);

        if !flags.contains(Flags::PRUNED) {
            unsafe { ptr::drop_in_place(self.evidence.as_mut_ptr()) };
        }
        if !flags.contains(Flags::DIRTY_FACT) {
            unsafe { ptr::drop_in_place(self.fact.get().cast::<T>()) };
        }
    }
}

#[cfg(test)]
mod tests {
    /*
    use super::*;

    use hoard::pile::TryPileMut;

    #[derive(Debug)]
    struct MMRLen {
        len: u64,
    }

    #[derive(Debug)]
    struct MMR {
        len: u64,
    }

    impl Commit for MMR {
        type Committed = MMR;

        fn commit(&self) -> Digest<MMR> {
            todo!()
        }
    }

    impl<Z> Fact<Z> for MMRLen {
        type Evidence = MMR;

        fn from_evidence(mmr: &MMR) -> Self {
            Self { len: mmr.len }
        }
    }

    #[test]
    fn test() {
        let pile = TryPileMut::default();

        let maybe = Maybe::<MMRLen, _>::new_in(MMR { len: 10 }, &pile);
        assert_eq!(maybe.len, 10);
        assert_eq!(maybe.len, 10);
    }
    */
}
