use core::cmp;
use core::convert::TryFrom;
use core::fmt;
use core::mem::{MaybeUninit, size_of};
use core::ops;
use core::ptr;
use core::slice;

#[repr(transparent)]
pub struct Bytes<T>(MaybeUninit<T>);

impl<T> Bytes<T> {
    pub const LEN: usize = size_of::<T>();

    /// Constructs a new `Bytes<T>` with all bytes set to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::bytes::Bytes;
    /// # use std::num::NonZeroU64;
    /// let b = Bytes::<NonZeroU64>::default();
    /// assert_eq!(b, [0, 0, 0, 0, 0, 0, 0, 0][..]);
    /// ```
    pub fn new() -> Self {
        Bytes(MaybeUninit::zeroed())
    }

    /// Constructs a new `Bytes<T>` from a `T` value.
    ///
    /// # Safety
    ///
    /// This is unsafe because `T` might contain uninitialized bytes.
    pub const unsafe fn from_value(value: T) -> Self {
        Bytes(MaybeUninit::new(value))
    }
}

impl<T> ops::Deref for Bytes<T> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self as *const _ as *const u8,
                                  size_of::<Self>())
        }
    }
}

impl<T> ops::DerefMut for Bytes<T> {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self as *mut _ as *mut u8,
                                      size_of::<Self>())
        }
    }
}

impl<T> Default for Bytes<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Bytes<T> {
    /// Clone is implemented even if `T` is not:
    ///
    /// ```
    /// # use hoard::bytes::Bytes;
    /// struct NonClone(u8);
    ///
    /// let b = Bytes::<NonClone>::default();
    /// let b2 = b.clone();
    /// assert_eq!(b, b2);
    /// ```
    ///
    /// But due to Rust limitations `Copy` is only implemented if `T: Copy`.
    fn clone(&self) -> Self {
        unsafe { ptr::read(self) }
    }
}
impl<T: Copy> Copy for Bytes<T> {}

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct TryFromSliceError;

impl<T> TryFrom<&'_ [u8]> for Bytes<T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <&Self as TryFrom<_>>::try_from(slice).map(Self::clone)
    }
}

impl<'a, T> TryFrom<&'a [u8]> for &'a Bytes<T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() == size_of::<T>() {
            Ok(unsafe { &*(slice as *const _ as *const _) })
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<'a, T> TryFrom<&'a mut [u8]> for &'a mut Bytes<T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &mut [u8]) -> Result<Self, Self::Error> {
        if slice.len() == size_of::<T>() {
            Ok(unsafe { &mut *(slice as *mut _ as *mut _) })
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<T> PartialEq for Bytes<T> {
    fn eq(&self, other: &Self) -> bool {
        &self[..] == &other[..]
    }
}
impl<T> PartialEq<[u8]> for Bytes<T> {
    fn eq(&self, other: &[u8]) -> bool {
        &self[..] == other
    }
}
impl<T> PartialEq<&'_ [u8]> for Bytes<T> {
    fn eq(&self, other: &&[u8]) -> bool {
        &self[..] == *other
    }
}
impl<T> Eq for Bytes<T> {}


impl<T> fmt::Debug for Bytes<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self[..], f)
    }
}
