//! Fact validation.

use core::any::type_name;
use core::marker::PhantomData;
use core::task;
use core::fmt;
use core::ops;

/// A proof that some fact is true.
pub struct Proof<T: ?Sized + Fact> {
    pub evidence: Option<T::Evidence>,
    pub fact: T,
}

pub enum Error<T: ?Sized + Fact> {
    Missing,
    Invalid(T::Error),
}
pub use Error::{Missing, Invalid};

impl<T: ?Sized + Fact> fmt::Debug for Error<T>
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

impl<T: Fact> Proof<T> {
    pub fn new(fact: T) -> Self {
        Self { fact, evidence: None, }
    }

    pub fn validate_in<'cx>(&self, cx: &'cx T::Context) -> Result<&Valid<'cx, T>,
                                                                  Error<T>>
    {
        match &self.evidence {
            None => Err(Missing),
            Some(evidence) => self.fact.validate_in(evidence, cx)
                                       .map_err(|err| Error::Invalid(err))
        }
    }

    pub fn validate<'cx>(&self) -> Result<&Valid<'cx, T>, Error<T>>
        where T::Context: Default,
    {
        let cx = T::Context::default();

        match &self.evidence {
            None => { return Err(Missing); },
            Some(evidence) => self.fact.validate_in(evidence, &cx)
                                       .map_err(|err| Invalid(err))?,
        };

        Ok(Valid::trust_ref(&self.fact))
    }

    pub fn poll<'cx>(&mut self, cx: &'cx T::Context, task_cx: &mut task::Context)
        -> task::Poll<Result<&Valid<'cx, T>, Error<T>>>
    {
        self.fact.poll(&mut self.evidence, cx, task_cx)
    }
}

pub trait Fact {
    type Evidence : Sized;
    type Context;
    type Error;

    fn validate_in<'cx>(&self, evidence: &Self::Evidence, cx: &'cx Self::Context)
        -> Result<&Valid<'cx, Self>, Self::Error>;

    fn poll<'cx>(&mut self, evidence: &mut Option<Self::Evidence>, cx: &'cx Self::Context, task_cx: &mut task::Context)
        -> task::Poll<Result<&Valid<'cx, Self>, Error<Self>>>
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

#[repr(transparent)]
pub struct Valid<'cx, T: ?Sized + Fact> {
    marker: PhantomData<&'cx T::Context>,
    value: T,
}

impl<T: Fact> Valid<'_,T> {
    pub fn trust(value: T) -> Self {
        Self { marker: PhantomData, value }
    }

    pub fn into_inner(this: Self) -> T {
        this.value
    }
}

impl<T: ?Sized + Fact> Valid<'_,T> {
    pub fn trust_ref(value: &T) -> &Self {
        unsafe {
            &*(value as *const T as *const Self)
        }
    }
}

/// `DerefMut` is *not* implemented, as changes to the value might invalidate it.
impl<T: ?Sized + Fact> ops::Deref for Valid<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}


impl<T: Fact> fmt::Debug for Valid<'_, T>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(type_name::<Self>())
            .field(&self.value)
            .finish()
    }
}

impl<T: Fact> fmt::Debug for Proof<T>
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

impl<T, A, B> Fact for (A,B)
where A: Fact<Evidence=T, Context=()>,
      B: Fact<Evidence=T, Context=()>,
{
}



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
