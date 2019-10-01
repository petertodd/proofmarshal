use core::num::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128};
use core::ptr::NonNull;

pub unsafe trait NonZero {}

/// Safe because ! can't actually be instantiated.
unsafe impl NonZero for ! {}

unsafe impl NonZero for NonZeroU8 {}
unsafe impl NonZero for NonZeroU16 {}
unsafe impl NonZero for NonZeroU32 {}
unsafe impl NonZero for NonZeroU64 {}
unsafe impl NonZero for NonZeroU128 {}

unsafe impl<'a,T: ?Sized> NonZero for &'a T {}
unsafe impl<'a,T: ?Sized> NonZero for &'a mut T {}
unsafe impl<'a,T: ?Sized> NonZero for NonNull<T> {}
