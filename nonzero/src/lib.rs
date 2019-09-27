/// Marker trait for non-zero types.

use core::num;
use core::ptr::NonNull;

use static_assertions::assert_eq_size;

/// Asserts that the bit representation of a value of this type is never all zeros, and that an
/// `Option<Self>` is the same size as `Self`.
///
/// # Safety
///
/// This is a less general concept than the niche-filling optimizations that Rust does! For
/// example `size_of::<Option<bool>>() == 1`, but `NonZero` can't be implemented as Rust uses `3`
/// as `None`.
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
