//! Contextual fact validation.

use std::marker::PhantomData;
use std::ops;
use std::task;

/// In-place validation.
pub trait Validate<Context : ?Sized> {
    type Error;

    /// Validate
    fn validate<'a>(&'a self, ctx: &mut Context) -> Result<&'a Valid<Self,Context>, Self::Error>;

    /// Try to make this a valid value by retreiving missing data.
    fn poll<'a>(&'a mut self, ctx: &mut Context, task_cx: &mut task::Context)
        -> task::Poll<Result<&'a Valid<Self,Context>, Self::Error>>
    {
        let _ = task_cx;
        task::Poll::Ready(self.validate(ctx))
    }
}

/// Wrapper marking things as valid in some context.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Valid<T: ?Sized, Context: ?Sized> {
    marker: PhantomData<Context>,
    trusted: T,
}

impl<T: ?Sized, X: ?Sized> Valid<T,X> {
    /// Trusts a reference.
    pub fn trust(trusted: &T) -> &Valid<T,X> {
        // Safe because Valid is #[repr(transparent)]
        unsafe { &*(trusted as *const _ as *const _) }
    }

    /// Trusts a reference.
    pub fn from_trusted(trusted: T) -> Valid<T,X>
        where T: Sized
    {
        Self {
            marker: PhantomData,
            trusted,
        }
    }
}

/// `DerefMut` is not implemented, as modifing the value could change its validity!
impl<T: ?Sized, X: ?Sized> ops::Deref for Valid<T,X> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.trusted
    }
}


