//! Marker traits.

use core::num;
use core::ptr::NonNull;

use static_assertions::assert_eq_size;

/// Types whose bit representation is never all-zero, *and* have niche-optimizations so that
/// `Option<Self>` is the same size as `Self`.
///
/// # Safety
///
/// This is a less general concept than the niche-filling optimizations that Rust does! For example
/// `size_of::<Option<bool>>() == 1`, but `NonZero` can't be implemented as Rust (usually) uses `3`
/// as `None`.
///
/// Since `NonZero` is used for in-place marshalling, which needs a well-defined `None`
/// representation. There is no guarantee that future versions of Rust will use `3` as `None` in
/// `Option<bool>`, thus we can't implement `NonZero` for `bool`.
pub unsafe trait NonZero {}

macro_rules! primitive_impls {
    ( $( $t:ty, )* ) => {
        $(
            assert_eq_size!($t, Option<$t>);
            unsafe impl NonZero for $t {}
        )*
    }
}

primitive_impls! {
    num::NonZeroU8, num::NonZeroU16, num::NonZeroU32, num::NonZeroU64, num::NonZeroU128,
    num::NonZeroI8, num::NonZeroI16, num::NonZeroI32, num::NonZeroI64, num::NonZeroI128,
}

assert_eq_size!(Box<()>, Option<Box<()>>);
unsafe impl<T: ?Sized> NonZero for Box<T> {}
unsafe impl<T: ?Sized> NonZero for &'_ T {}
unsafe impl<T: ?Sized> NonZero for &'_ mut T {}
unsafe impl<T: ?Sized> NonZero for NonNull<T> {}

assert_eq_size!(Option<!>, !);
unsafe impl NonZero for ! {}

macro_rules! array_impls {
    ($($N:literal)+) => {
        $(
            assert_eq_size!([Box<()>; $N], Option<[Box<()>; $N]>);
            unsafe impl<T: NonZero> NonZero for [T; $N] {}
        )+
    }
}

// Note how [T;0] is *not* implemented: NonZero types have to actually have some non-zero bytes in
// them.
array_impls! {
        1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}
