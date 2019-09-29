//! Fact validation.

use std::marker::PhantomData;
use std::ops;

/// Wrapper for refutable facts that are only valid in a certain context.
#[repr(transparent)]
pub struct Valid<T: ?Sized, Context = ()> {
    marker: PhantomData<Context>,
    value: T,
}

impl<T, Context> Valid<T,Context> {
    pub fn trust(value: T) -> Self {
        Self { marker: PhantomData, value }
    }

    pub fn into_inner(this: Self) -> T {
        this.value
    }
}

impl<T: ?Sized, Context> Valid<T,Context> {
    pub fn trust_ref(value: &T) -> &Self {
        unsafe {
            &*(value as *const T as *const Self)
        }
    }
}

/// `DerefMut` is *not* implemented, as changes to the value might invalidate it.
impl<T,Context> ops::Deref for Valid<T,Context> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}
