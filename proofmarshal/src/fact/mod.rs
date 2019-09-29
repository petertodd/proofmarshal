//! Fact validation.

use core::any::type_name;
use core::marker::PhantomData;
use core::task;
use core::fmt;
use core::ops::{Deref, DerefMut};

use std::borrow::Cow;

mod lazy;
use self::lazy::Lazy;

use crate::arena::{Own, Ptr, MutPtr, Load, Alloc};

/// A fact that can be directly derived from evidence.
pub trait Fact<P = ()> : Sized {
    type Evidence : Load<P>;

    /// It must be always possible to derive the fact from the evidence without any further state.
    fn derive(evidence: &Self::Evidence) -> Self;
}

/// A proof that some fact is true.
#[derive(Debug)]
pub struct Proof<T: Fact<P>, P: Ptr = ()> {
    trusted: bool,
    evidence: Option<Own<T::Evidence, P>>,
    fact: Lazy<T>,
}

impl<T: Fact<P>, P: Ptr> Proof<T,P> {
    /// Creates a `Proof` from the evidence.
    ///
    /// The fact will be lazily computed from the evidence.
    pub fn new(evidence: T::Evidence) -> Self
        where P: Default
    {
        Self::new_in(evidence, &mut P::allocator())
    }

    /// Creates a `Proof`, allocating the evidence with the provided allocator.
    pub fn new_in(evidence: T::Evidence, alloc: &mut impl Alloc<Ptr=P>) -> Self {
        Self {
            trusted: false,
            evidence: Some(Own::new_in(evidence, alloc)),
            fact: Lazy::uninit(),
        }
    }

    /// Creates a `Proof` from a proven fact.
    pub fn from_fact(fact: T) -> Self {
        Self {
            trusted: false,
            evidence: None,
            fact: Lazy::new(fact),
        }
    }

    /*
    pub fn unprune_then<'p, F, R>(&self, f: F) -> R
        where F: FnOnce(Option<Cow<'_, T>>) -> R
    {
        match self.evidence {
            None => f(None),
        }
    }

    pub fn unprune_mut(&mut self) -> Option<&mut T::Evidence> {
        unimplemented!()
    }
    */
}

impl<T: Fact<P>, P: Ptr> Deref for Proof<T,P> {
    type Target = T;

    fn deref(&self) -> &T {
        if let Some(r) = self.fact.get() {
            r
        } else {
            let own = self.evidence.as_ref()
                          .expect("Evidence available if derived fact uninitialized");

            let fact = T::derive(own.get().deref());

            // It's possible the set will fail if another thread is co-currently dereferencing this
            // fact. That's ok and can be ignored.
            let _ = self.fact.try_set(fact);

            self.fact.get().expect("Derived fact available after setting it")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    #[derive(Debug)]
    struct Block {
        value: usize,
        next: Option<Proof<Blockchain>>,
    }

    #[derive(Debug)]
    struct Blockchain {
        digest: [u8;32],
        sum: Option<usize>,
    }

    impl Fact for Blockchain {
        type Evidence = Block;

        fn derive(block: &Block) -> Blockchain {
            Blockchain {
                digest: [0;32],
                sum: None,
            }
        }
    }

    #[test]
    fn test() {
        let block = Block {
            value: 0,
            next: None,
        };

        let mut chain_proof = Proof::<Blockchain>::from_evidence(block);

        dbg!(chain_proof);
    }
    */
}


/*
/// A mutable proof that some fact is true.
///
/// The opposite of a `Proof`: the evidence is always available, and the derived fact is
/// constructed on demand.
pub struct ProofMut<T: ?Sized + Fact<A>, A> {
    evidence: T::Evidence,
    fact: Cell<Option<T>>,
}





pub enum Error<T: ?Sized + Fact<A>, A> {
    Missing,
    Invalid(T::Error),
}
pub use Error::{Missing, Invalid};

*/





/*

impl<T: ?Sized + Fact<A>, A> fmt::Debug for Error<T,A>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Missing => f.debug_tuple("Missing")
                               .finish(),
            Error::Invalid(err) => f.debug_tuple("Invalid")
                                    .field(err)
                                    .finish()
        }
    }
}

impl<T: Fact<A>, A> Proof<T,A> {
    pub fn new(fact: T) -> Self {
        Self { fact, evidence: None, }
    }

    pub fn validate_in<'cx>(&self, cx: &'cx A) -> Result<&Valid<'cx, T>,
                                                         Error<T,A>>
    {
        match &self.evidence {
            None => Err(Missing),
            Some(evidence) => self.fact.validate_in(evidence, cx)
                                       .map_err(|err| Error::Invalid(err))
        }
    }

    pub fn validate<'cx>(&self) -> Result<&Valid<'cx, T>, Error<T,A>>
        where A: Default,
    {
        let cx = A::default();

        match &self.evidence {
            None => { return Err(Missing); },
            Some(evidence) => self.fact.validate_in(evidence, &cx)
                                       .map_err(|err| Invalid(err))?,
        };

        Ok(Valid::trust_ref(&self.fact))
    }

    pub fn poll<'cx>(&mut self, cx: &'cx A, task_cx: &mut task::Context)
        -> task::Poll<Result<&Valid<'cx, T>, Error<T,A>>>
    {
        self.fact.poll(&mut self.evidence, cx, task_cx)
    }
}

pub trait Fact<A> {
    type Evidence : Sized;
    type Error;

    fn validate_in<'cx>(&self, evidence: &Self::Evidence, cx: &'cx A)
        -> Result<&Valid<'cx, Self>, Self::Error>;

    fn poll<'cx>(&mut self, evidence: &mut Option<Self::Evidence>, cx: &'cx A, task_cx: &mut task::Context)
        -> task::Poll<Result<&Valid<'cx, Self>, Error<Self,A>>>
    {
        let _ = task_cx;
        task::Poll::Ready(
            match evidence {
                None => Err(Missing),
                Some(evidence) => self.validate_in(evidence, cx)
                                      .map_err(|err| Invalid(err))
            }
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Contextless;

#[repr(transparent)]
pub struct Valid<'cx, T: ?Sized> {
    marker: PhantomData<&'cx ()>,
    value: T,
}

impl<T> Valid<'_,T> {
    pub fn trust(value: T) -> Self {
        Self { marker: PhantomData, value }
    }

    pub fn into_inner(this: Self) -> T {
        this.value
    }
}

impl<T: ?Sized> Valid<'_,T> {
    pub fn trust_ref(value: &T) -> &Self {
        unsafe {
            &*(value as *const T as *const Self)
        }
    }
}

/// `DerefMut` is *not* implemented, as changes to the value might invalidate it.
impl<T: ?Sized> ops::Deref for Valid<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}


/*
impl<T: Fact> fmt::Debug for Valid<'_, T>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(type_name::<Self>())
            .field(&self.value)
            .finish()
    }
}
*/

impl<T: Fact<A>, A> fmt::Debug for Proof<T,A>
where T: fmt::Debug,
      T::Evidence: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("evidence", &self.evidence)
            .field("fact", &self.fact)
            .finish()
    }
}

impl<A, F1, F2> Fact<A> for (F1,F2)
where F1: Fact<A>,
      F2: Fact<A, Evidence=<F1 as Fact<A>>::Evidence>,
{
    type Evidence = F1::Evidence;
    type Error = !;

    fn validate_in<'cx>(&self, evidence: &Self::Evidence, cx: &'cx A)
        -> Result<&Valid<'cx, Self>, Self::Error>
    {
        unimplemented!()
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    pub struct Sum(usize);

    #[derive(Debug)]
    pub struct SumError;

    impl Fact for Sum {
        type Evidence = (usize, usize);
        type Context = ();
        type Error = SumError;

        fn validate_in<'cx>(&self, (lhs, rhs): &Self::Evidence, cx: &'cx Self::Context)
            -> Result<&Valid<'cx, Self>, Self::Error>
        {
            let actual_sum = lhs.checked_add(*rhs).ok_or(SumError)?;

            if actual_sum == self.0 {
                Ok(Valid::trust_ref(self))
            } else {
                Err(SumError)
            }
        }
    }

    #[test]
    fn test() {
        let mut sum_proof = dbg!(Proof::new(Sum(100)));
        sum_proof.evidence = Some((50, 50));

        let _ = dbg!(sum_proof.validate());
    }
}
*/
*/
