//! In-place coercions.

use static_assertions::assert_impl_all;

use core::alloc::Layout;
use core::any::type_name;
use core::mem::ManuallyDrop;

mod array;
pub use self::array::*;

mod scalars;
pub use self::scalars::*;
pub unsafe trait TryCoerce<T> : Sized {
    type Error;

    fn try_coerce_ptr(this: &Self) -> Result<*const T, Self::Error> {
        assert_eq!(Layout::new::<Self>(), Layout::new::<T>(),
                   "{} can-not implement TryCoerce<{}>: layouts differ",
                   type_name::<Self>(), type_name::<T>());

        Ok(this as *const Self as *const T)
    }

    fn try_coerce(self) -> Result<T, Self::Error> {
        assert_eq!(Layout::new::<Self>(), Layout::new::<T>(),
                   "{} can-not implement TryCoerce<{}>: layouts differ",
                   type_name::<Self>(), type_name::<T>());

        let r_ptr = Self::try_coerce_ptr(&self)?;

        unsafe {
            let r = r_ptr.read();
            drop(self);
            Ok(r)
        }
    }
}

unsafe impl<T> TryCoerce<T> for ! {
    type Error = !;
    fn try_coerce_ptr(this: &Self) -> Result<*const T, Self::Error> {
        match *this {}
    }

    fn try_coerce(self) -> Result<T, Self::Error> {
        match self {}
    }
}

pub unsafe trait Coerce<T> : Sized {
    fn coerce_ptr(this: &Self) -> *const T;
    fn coerce(self) -> T;
}

unsafe impl<U, T: TryCoerce<U>> Coerce<U> for T
where T::Error: Into<!>
{
    fn coerce_ptr(this: &Self) -> *const U {
        assert_eq!(Layout::new::<Self>(), Layout::new::<U>(),
                   "{} can-not implement TryCoerce<{}>: layouts differ",
                   type_name::<Self>(), type_name::<U>());

        match Self::try_coerce_ptr(this) {
            Ok(r_ptr) => r_ptr,
            Err(never) => match Into::<!>::into(never) {},
        }
    }

    fn coerce(self) -> U {
        assert_eq!(Layout::new::<Self>(), Layout::new::<U>(),
                   "{} can-not implement TryCoerce<{}>: layouts differ",
                   type_name::<Self>(), type_name::<U>());

        let r_ptr = Self::coerce_ptr(&self);

        unsafe {
            let r = r_ptr.read();
            drop(self);
            r
        }
    }
}

macro_rules! unsafe_impl_coerce {
    () => {};
    ($t:ty => $u:ty) => {
        unsafe impl TryCoerce<$u> for $t {
            type Error = !;
        }
    };

    ($t:ty => { $($u:ty),+ $(,)?}) => {
        $(
            unsafe_impl_coerce!($t => $u);
        )+
    };

    ($t:ty => $u:tt; $($rest_t:ty => $rest_u:tt;)*) => {
        unsafe_impl_coerce!($t => $u);
        unsafe_impl_coerce!($($rest_t => $rest_u;)*);
    };
}

unsafe_impl_coerce! {
    () => ();
    bool => {bool, u8};
}

assert_impl_all!(!: TryCoerce<!>, Coerce<!>);
assert_impl_all!(bool: Coerce<bool>, Coerce<u8>);
