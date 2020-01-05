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
pub struct Maybe<T, Z: Zone, E: ?Sized + Pointee = <T as Fact<Z>>::Evidence> {
    flags: AtomicU8,
    fact: UnsafeCell<MaybeUninit<T>>,
    evidence: MaybeUninit<OwnedPtr<E, Z>>,
}

bitflags::bitflags! {
    pub struct Flags: u8 {
        const HAVE_EVIDENCE = 0b001;
        const HAVE_FACT     = 0b010;
        const LOCKED        = 0b100;
    }
}

impl From<Flags> for AtomicU8 {
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

impl<T, Z: Zone, E: ?Sized + Pointee> Drop for Maybe<T, Z, E> {
    fn drop(&mut self) {
        let flags = self.load_flags(Ordering::Relaxed);
        assert!(!flags.contains(Flags::LOCKED));

        if flags.contains(Flags::HAVE_EVIDENCE) {
            unsafe { ptr::drop_in_place(self.evidence.as_mut_ptr()) };
        }
        if flags.contains(Flags::HAVE_FACT) {
            unsafe { ptr::drop_in_place(self.fact.get().cast::<T>()) };
        }
    }
}

impl<Z: Zone, T, E: ?Sized + Pointee> Maybe<T, Z, E> {
    fn load_flags(&self, ordering: Ordering) -> Flags {
        let flags = self.flags.load(ordering);
        match Flags::from_bits(flags) {
            Some(flags) => {
                flags
            },
            None => {
                unreachable!("invalid flags: {:b}", flags)
            }
        }
    }

    pub fn from_fact(fact: T) -> Self {
        Self {
            flags: Flags::HAVE_FACT.into(),
            fact: MaybeUninit::new(fact).into(),
            evidence: MaybeUninit::uninit(),
        }
    }

    pub fn from_evidence(evidence: OwnedPtr<E, Z>) -> Self {
        Self {
            flags: Flags::HAVE_EVIDENCE.into(),
            fact: MaybeUninit::uninit().into(),
            evidence: MaybeUninit::new(evidence),
        }
    }

    pub unsafe fn new_unchecked(fact: T, evidence: OwnedPtr<E, Z>) -> Self {
        Self {
            flags: (Flags::HAVE_FACT | Flags::HAVE_EVIDENCE).into(),
            fact: MaybeUninit::new(fact).into(),
            evidence: MaybeUninit::new(evidence),
        }
    }

    pub fn try_get_fact(&self) -> Result<&T, &OwnedPtr<E, Z>> {
        let flags = self.load_flags(Ordering::Relaxed);
        if flags.contains(Flags::HAVE_FACT) {
            Ok(unsafe { &*self.fact.get().cast::<T>() })
        } else if flags.contains(Flags::HAVE_EVIDENCE) {
            Err(unsafe { &*self.evidence.as_ptr() })
        } else {
            unreachable!("missing both fact and evidence")
        }
    }

    pub fn try_get_evidence(&self) -> Result<&OwnedPtr<E, Z>, &T> {
        match self.try_get_fact() {
            Ok(fact) => Err(fact),
            Err(evidence) => Ok(evidence),
        }
    }
}

impl<Z: Zone, T: Fact<Z>> Maybe<T, Z> {
    /// Gets access to the proven fact, trusting that it's true.
    pub fn trust(&self) -> &T {
        match self.try_get_fact() {
            Ok(fact) => fact,
            Err(_evidence) => {
                todo!()
            }
        }
    }

    /*
    /// Tries to get access to the evidence.
    pub fn try_unprune(&self, zone: &Z) -> Result<&T::Evidence, Z::Error> {
        todo!()
    }

    /// Tries to get mutable access to the evidence.
    pub fn try_unprune_mut(&mut self, zone: &Z) -> Result<&mut T::Evidence, Z::Error> {
        todo!()
    }
    */
}

impl<Z: Zone, T, E> fmt::Debug for Maybe<T, Z, E>
where T: fmt::Debug,
      E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let state = self.load_flags(Ordering::Relaxed);
        f.debug_struct("Maybe")
            .field("state", &state)
            .field("fact", &self.try_get_fact().ok())
            .field("evidence", &self.try_get_evidence().ok())
            .finish()
    }
}

impl<Z: Zone, T: Fact<Z> + Verbatim> Verbatim for Maybe<T, Z> {
    const LEN: usize = <T as Verbatim>::LEN;

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        self.trust().encode_verbatim(dst)
    }
}

/*
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
*/

#[cfg(test)]
mod tests {
    use super::*;

    use dropcheck::DropCheck;

    #[test]
    fn test_fact_drop() {
        let dropcheck = DropCheck::new();
        Maybe::<_, !, !>::from_fact(dropcheck.token());
    }
}
