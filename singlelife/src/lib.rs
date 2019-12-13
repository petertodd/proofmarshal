#![feature(arbitrary_self_types)]

use core::borrow::{Borrow, BorrowMut};
use core::fmt;
use core::marker::PhantomData;
use core::ops;

/// A wrapper that guarantees that a value is a singleton with respect to a *lifetime*.
///
/// The existence of a `Unique<'u, T>` guarantees that there exists exactly one `T` value with
/// lifetime `'u`. Secondly, `Unique` is invariant over `'u`, making `Unique<'u, T>` a distinct
/// type from `Unique<'v, T>`. That means that the following won't compile, as `outer` and `inner`
/// are treated as incompatible types:
///
/// ```compile_fail
/// # use singlelife::Unique;
/// Unique::new((), |outer| {
///     Unique::new((), |inner| {
///         outer == inner
///     })
/// });
/// ```
#[derive(PartialEq,Eq,PartialOrd,Ord,Hash)]
#[repr(transparent)]
pub struct Unique<'u, T: 'u + ?Sized> {
    marker: PhantomData<fn(&'u T) -> &'u T>,
    value: T,
}

/// A globally unique singleton value.
pub type Singleton<T> = Unique<'static, T>;

impl<T: ?Sized> Unique<'_, T> {
    /// Creates a new `Unique<'u, T>`.
    #[inline(always)]
    pub fn new<R>(value: T, f: impl for<'u> FnOnce(Unique<'u, T>) -> R) -> R
        where T: Sized
    {
        unsafe {
            f(Unique::new_unchecked(value))
        }
    }

    /// Makes a reference unique.
    #[inline(always)]
    pub fn from_ref<'a, R>(r: &'a T, f: impl for<'u> FnOnce(&'a Unique<'u, T>) -> R) -> R {
        unsafe {
            f(Unique::from_ref_unchecked(r))
        }
    }

    /// Makes a mutable reference unique.
    #[inline(always)]
    pub fn from_mut<'a, R>(r: &'a mut T, f: impl for<'u> FnOnce(&'a mut Unique<'u, T>) -> R) -> R {
        unsafe {
            f(Unique::from_mut_unchecked(r))
        }
    }
}

impl<'u, T> Unique<'u, T> {
    /// Unsafely creates a new `Unique<'u,T>`.
    ///
    /// # Safety
    ///
    /// The callee must guarantee that only one unique value exists for the given lifetime at all
    /// times.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(arbitrary_self_types)]
    /// # use singlelife::Unique;
    ///
    /// struct Foo(u8);
    /// struct Bar(Foo);
    ///
    /// impl Bar {
    ///     fn new<'u>(foo: Unique<'u, Foo>) -> Unique<'u, Self> {
    ///         let bar = Bar(Unique::into_inner(foo));
    ///
    ///         // Safe because foo was unique.
    ///         unsafe { Unique::new_unchecked(bar) }
    ///     }
    ///
    ///     fn take<'u>(self: Unique<'u, Self>) -> Unique<'u, Foo> {
    ///         // Safe because we were created from a unique Foo.
    ///         let this = Unique::into_inner(self);
    ///         unsafe { Unique::new_unchecked(this.0) }
    ///     }
    /// }
    /// ```
    #[inline(always)]
    pub unsafe fn new_unchecked(value: T) -> Self {
        Self {
            marker: PhantomData,
            value,
        }
    }

    /// Unwraps the `Unique<'u, T>`, returning the inner value.
    #[inline(always)]
    pub fn into_inner(unique: Self) -> T {
        unique.value
    }

    /// Constructs a `Unique` reference by mapping the interior value.
    ///
    /// # Safety
    ///
    /// The callee must guarantee that the returned value is unique.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(arbitrary_self_types)]
    /// # use singlelife::Unique;
    ///
    /// struct Foo(u8);
    /// struct Bar(Foo);
    ///
    /// impl Bar {
    ///     fn new<'u>(foo: Unique<'u, Foo>) -> Unique<'u, Self> {
    ///         let bar = Bar(Unique::into_inner(foo));
    ///
    ///         // Safe because foo was unique.
    ///         unsafe { Unique::new_unchecked(bar) }
    ///     }
    ///
    ///     fn get<'a, 'u>(self: &'a Unique<'u, Self>) -> &'a Unique<'u, Foo> {
    ///         // Safe because we were created from a unique Foo.
    ///         unsafe {
    ///             Unique::map_unchecked(self, |this| &this.0)
    ///         }
    ///     }
    /// }
    /// ```
    pub unsafe fn map_unchecked<R: ?Sized>(unique: &Self, f: impl FnOnce(&T) -> &R) -> &Unique<'u, R> {
        let r = f(&unique.value);
        Unique::from_ref_unchecked(r)
    }

    /// `map_unchecked` but mutable.
    pub unsafe fn map_unchecked_mut<R: ?Sized>(unique: &mut Self, f: impl FnOnce(&mut T) -> &mut R) -> &mut Unique<'u, R> {
        let r = f(&mut unique.value);
        Unique::from_mut_unchecked(r)
    }
}

impl<'u, T: ?Sized> Unique<'u, T> {
    /// Unsafely makes a reference unique.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(arbitrary_self_types)]
    /// # use singlelife::Unique;
    ///
    /// struct Foo(u8);
    /// struct Bar(Foo);
    ///
    /// impl Bar {
    ///     fn new<'u>(foo: Unique<'u, Foo>) -> Unique<'u, Self> {
    ///         let bar = Bar(Unique::into_inner(foo));
    ///
    ///         // Safe because foo was unique.
    ///         unsafe { Unique::new_unchecked(bar) }
    ///     }
    ///
    ///     fn get<'a, 'u>(self: &'a Unique<'u, Self>) -> &'a Unique<'u, Foo> {
    ///         // Safe because we were created from a unique Foo.
    ///         unsafe { Unique::from_ref_unchecked(&self.0) }
    ///     }
    /// }
    /// ```
    #[inline(always)]
    pub unsafe fn from_ref_unchecked<'a>(r: &'a T) -> &'a Unique<'u, T> {
        // Safe because #[repr(transparent]
        &*(r as *const T as *const Unique<T>)
    }

    /// Unsafely makes a mutable reference unique.
    #[inline(always)]
    pub unsafe fn from_mut_unchecked<'a>(r: &'a mut T) -> &'a mut Unique<'u, T> {
        // Safe because #[repr(transparent]
        &mut *(r as *mut T as *mut Unique<T>)
    }
}

impl<T: ?Sized> ops::Deref for Unique<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: ?Sized> ops::DerefMut for Unique<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: ?Sized> Borrow<T> for Unique<'_, T> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        &self.value
    }
}

impl<T: ?Sized> BorrowMut<T> for Unique<'_, T> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Unique<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Unique<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value.fmt(f)
    }
}

#[macro_export]
macro_rules! unique {
    ( | | $f:expr ) => { $f };
    ( | $name:ident $(, $rest:ident)* | $f:expr ) => {
        $crate::Unique::new($name, | $name |
            $crate::unique!(| $($rest),* | $f)
        )
    };
    ( | &$name:ident $(, $rest:ident)* | $f:expr ) => {
        $crate::Unique::from_ref(&$name, | $name |
            $crate::unique!(| $($rest),* | $f)
        )
    };
    ( | &mut $name:ident $(, $rest:ident)* | $f:expr ) => {
        $crate::Unique::from_mut(&mut $name, | $name |
            $crate::unique!(| $($rest),* | $f)
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifetime() {
        fn foo<'a, 'b: 'a>(slice: Unique<'a, &'b [u8]>) -> &'b [u8] {
            *slice
        }

        let v = vec![];
        let slice = &v[..];

        Unique::new(slice, |slice| {
            foo(slice)
        });
    }

    struct Foo {
        n: u8,
    }

    impl Foo {
        fn new(n: u8) -> Self {
            Self { n }
        }

        fn get<'a,'u>(self: &'a Unique<'u, Self>) -> &'a Unique<'u, u8> {
            unsafe { Unique::from_ref_unchecked(&self.n) }
        }
    }

    #[test]
    fn test() {
        Unique::new(Foo::new(1), |foo| {
            Unique::new(Foo::new(1), |bar| {
                assert_eq!(foo.get(), foo.get());
                assert_eq!(bar.get(), bar.get());
            })
        })
    }

    #[test]
    fn test_unique_macro() {
        let foo = Foo::new(1);
        let bar = Foo::new(2);
        unique!(|foo, bar| {
            assert_eq!(**foo.get(), 1);
            assert_eq!(**bar.get(), 2);
        });

        let foo = Foo::new(1);
        let bar = Foo::new(2);
        unique!(|&foo, bar| {
            assert_eq!(**foo.get(), 1);
            assert_eq!(**bar.get(), 2);
        });

        let bar = Foo::new(2);
        unique!(|&foo, bar| {
            assert_eq!(**foo.get(), 1);
            assert_eq!(**bar.get(), 2);
        });
    }
}
