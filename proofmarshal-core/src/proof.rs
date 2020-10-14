use std::mem::MaybeUninit;
use std::cell::Cell;
use std::borrow::BorrowMut;

use hoard::bag::Bag;
use hoard::zone::{Zone, Ptr, Alloc};
use hoard::pointee::Pointee;
use hoard::owned::Take;

pub struct Proof<T, Z = (), P: Ptr = <Z as Zone>::Ptr, U: ?Sized + Pointee = <T as Prove<Z, P>>::Evidence> {
    maybe_fact: Cell<Option<T>>,

    evidence: EvidenceState<U, Z, P>,
}

#[derive(Debug)]
enum EvidenceState<U: ?Sized + Pointee, Z, P: Ptr> {
    Missing,
    Avail(Bag<U, Z, P>),
    Pruned(Bag<U, Z, P>),
}

pub trait Prove<Z, P = <Z as Zone>::Ptr> {
    type Evidence : ?Sized + Pointee;

    fn derive(evidence: &Self::Evidence) -> Self;
}

impl<T: Copy, Z, P: Ptr, U> Proof<T, Z, P, U> {
    pub fn trust(fact: T) -> Self {
        Self {
            maybe_fact: Some(fact).into(),
            evidence: EvidenceState::Missing,
        }
    }
}

impl<T: Copy, Z: Zone> Proof<T, Z>
where T: Prove<Z>
{
    pub fn from_evidence_in(evidence: impl Take<T::Evidence>, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc
    {
        Self {
            maybe_fact: None.into(),
            evidence: EvidenceState::Avail(zone.borrow_mut().alloc(evidence)),
        }
    }
}
