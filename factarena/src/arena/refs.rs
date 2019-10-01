//! Generic references wrapping borrowed or owned data.

use core::borrow::Borrow;
use core::fmt;
use core::ops::Deref;

use super::*;

/// Immutable reference.
pub struct Ref<'a, T: ?Sized + Type<A>, A: Arena>(State<'a,T,A>);


enum State<'a, T: ?Sized + Type<A>, A: Arena> {
    /// Borrowed from memory
    Borrowed(&'a T),

    /// Owned
    Owned(T::RefOwned),
}


impl<'a, T: ?Sized + Type<A>, A: Arena> Ref<'a, T, A> {
    pub fn from_borrowed(borrowed: &'a T) -> Self {
        Ref(State::Borrowed(borrowed))
    }

    pub fn from_owned(owned: T::RefOwned) -> Self {
        Ref(State::Owned(owned))
    }
}


impl<'a, T: ?Sized + Type<A>, A: Arena> Deref for Ref<'a,T,A> {
    type Target = T;

    fn deref(&self) -> &T {
        match &self.0 {
            State::Borrowed(borrowed) => borrowed,
            State::Owned(owned) => owned.borrow(),
        }
    }
}

impl<'a, T: ?Sized + Type<A>, A: Arena> fmt::Debug for Ref<'a,T,A>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            State::Borrowed(borrowed) => {
                f.debug_tuple("Ref::Borrowed")
                    .field(&borrowed)
                    .finish()
            },
            State::Owned(owned) => {
                f.debug_tuple("Ref::Owned")
                    .field(&owned.borrow())
                    .finish()
            },
        }
    }
}

/*
impl<'a, T: ?Sized + Pointee> Borrow<T> for Ref<'a, T> {
    fn borrow(&self) -> &T {
        self.deref()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let v = 42u8;

        let r = Ref::from_borrowed(&v);
        dbg!(r);

        dbg!(Ref::<u8>::from_owned(42u8));
    }
}
*/
