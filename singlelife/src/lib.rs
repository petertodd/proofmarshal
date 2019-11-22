#![feature(arbitrary_self_types)]

use core::ops;

#[derive(Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
#[repr(transparent)]
pub struct Unique<T: ?Sized>(T);

impl<T> Unique<T> {
    #[inline(always)]
    pub fn new<R>(value: T, f: impl for<'a> FnOnce(&'a mut Unique<T>) -> R) -> R {
        unsafe {
            let mut unique = Self::new_unchecked(value);
            f(&mut unique)
        }
    }

    #[inline(always)]
    pub unsafe fn new_unchecked(value: T) -> Unique<T> {
        Self(value)
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: ?Sized> Unique<T> {
    #[inline(always)]
    pub fn new_ref<R>(r: &mut T, f: impl for<'a> FnOnce(&'a mut Unique<T>) -> R) -> R {
        unsafe {
            f(Self::new_ref_unchecked(r))
        }
    }

    #[inline(always)]
    pub unsafe fn new_ref_unchecked(r: &mut T) -> &mut Unique<T> {
        &mut *(r as *mut T as *mut Unique<T>)
    }
}

impl<T: ?Sized> ops::Deref for Unique<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> ops::DerefMut for Unique<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/*
#[macro_export]
macro_rules! unique {
    ( $name:ident, $($rest:ident ),* => $f:expr ) => {{
        let $name = unsafe { $crate::Unique::new_unchecked($name) };
        { $f }
    }};
    ( mut $name:ident $(, $rest:ident ),* => $f:expr ) => {{
        let mut $name = unsafe { $crate::Unique::new_unchecked($name) };
        { $f }
    }};
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    struct Foo {
        value: u8,
    }

    impl Foo {
        fn new(value: u8) -> Self {
            Self { value }
        }

        fn get(&self) -> u8 {
            self.value
        }

        fn get_unique<'a>(self: &'a mut Unique<Self>) -> &'a mut Unique<u8> {
            unsafe { Unique::new_ref_unchecked(&mut self.value) }
        }
    }

    #[test]
    fn test() {
        let foo = Foo::new(10);

        Unique::new(foo, |foo| {
            assert_eq!(foo.get(), 10);
            assert_eq!(**foo.get_unique(), 10);
        });
    }
}
