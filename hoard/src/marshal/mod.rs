//! In-place data marshalling.
//!
//!

use core::cell::UnsafeCell;
use core::marker::PhantomData;

pub mod blob;
pub mod primitive;
pub mod en;
pub mod de;
pub mod impls;

/// Types that don't contain interior mutability.
pub unsafe auto trait Freeze {}

impl<T: ?Sized> !Freeze for UnsafeCell<T> {}
unsafe impl<T: ?Sized> Freeze for PhantomData<T> {}
unsafe impl<T: ?Sized> Freeze for *const T {}
unsafe impl<T: ?Sized> Freeze for *mut T {}
unsafe impl<T: ?Sized> Freeze for &T {}
unsafe impl<T: ?Sized> Freeze for &mut T {}

pub mod prelude {
    pub use super::en::*;
    pub use super::de::*;
    pub use super::blob::*;
}
