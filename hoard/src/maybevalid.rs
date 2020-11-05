use crate::owned::{Ref, IntoOwned};

/// A wrapper type for values that may not be fully valid.
#[derive(Debug)]
#[repr(transparent)]
pub struct MaybeValid<T: ?Sized>(T);

impl<U: ?Sized, T: ?Sized> AsRef<MaybeValid<U>> for MaybeValid<T>
where T: AsRef<U>
{
    fn as_ref(&self) -> &MaybeValid<U> {
        let u_ref: &U = self.0.as_ref();
        u_ref.into()
    }
}

impl<T> MaybeValid<T> {
    pub const fn new(inner: T) -> Self {
        MaybeValid(inner)
    }

    /// Extracts the wrapped value, trusting it to be valid.
    pub fn trust(self) -> T {
        self.0
    }
}

impl<T: ?Sized> MaybeValid<T> {
    /// Extracts a reference to the wrapped value, trusting it to be valid.
    pub fn trust_ref(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for MaybeValid<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<'a, T: ?Sized> From<&'a T> for &'a MaybeValid<T> {
    fn from(inner_ref: &'a T) -> Self {
        // SAFETY: #[repr(transparent)]
        unsafe { &* (inner_ref as *const T as *const Self) }
    }
}

impl<'a, T: ?Sized + IntoOwned> From<&'a T> for MaybeValid<Ref<'a, T>> {
    fn from(inner_ref: &'a T) -> Self {
        Self(Ref::Borrowed(inner_ref))
    }
}
