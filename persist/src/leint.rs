/// Little-endian integers.

use core::cmp;
use core::marker::PhantomData;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::mem;
use core::num::{
    NonZeroU8,   NonZeroI8,
    NonZeroU16,  NonZeroI16,
    NonZeroU32,  NonZeroI32,
    NonZeroU64,  NonZeroI64,
    NonZeroU128, NonZeroI128,
};
use core::slice;

use nonzero::NonZero;

use super::{Persist, MaybeValid, UninitBytes};

/// A little-endian integer.
///
/// The actual memory representation of a `Le<T>` will be little-endian regardless of platform
/// endianness.
#[repr(packed)]
pub struct Le<T: sealed::ToFromLe>(T);

mod sealed {
    use super::*;

    pub trait ToFromLe
        : 'static + Copy + Eq + Ord + fmt::Display + fmt::Debug
    {
        fn to_le(this: Self) -> Self;
        fn from_le(le_this: Self) -> Self;
    }
}
use self::sealed::ToFromLe;

impl<T: ToFromLe> Le<T> {
    #[inline(always)]
    pub fn new(n: T) -> Self {
        Le(T::to_le(n))
    }

    #[inline(always)]
    pub fn get(self) -> T {
        T::from_le(self.0)
    }
}

impl<T: ToFromLe> From<T> for Le<T> {
    #[inline(always)]
    fn from(n: T) -> Self {
        Le::new(n)
    }
}

impl<T: ToFromLe> fmt::Debug for Le<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Le({:?})", self.get())
    }
}
impl<T: ToFromLe> fmt::Display for Le<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

impl<T: ToFromLe> Clone for Le<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Le(self.0)
    }
}
impl<T: ToFromLe> Copy for Le<T> {}

impl<T: ToFromLe + Default> Default for Le<T> {
    #[inline(always)]
    fn default() -> Self {
        Le::from(T::default())
    }
}

impl<T: ToFromLe> Hash for Le<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::hash_slice(slice::from_ref(self), state)
    }
    #[inline]
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H) {
        unsafe {
            let buf: &[u8] = slice::from_raw_parts(data.as_ptr() as *const u8,
                                                   data.len() * mem::size_of::<Self>());
            state.write(buf)
        }
    }
}



macro_rules! impl_ints {
    ( $( $t:ident, )* ) => {
        $(
            impl_tofromle!($t, $t);

            unsafe impl Persist for Le<$t> {
                type Error = core::convert::Infallible;

                #[inline(always)]
                fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
                    unsafe { Ok(maybe.assume_valid()) }
                }

                #[inline(always)]
                fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
                    let buf = unsafe { slice::from_raw_parts(self as *const _ as *const u8,
                                                             mem::size_of::<Self>()) };
                    dst.write_bytes(buf);
                    dst.done()
                }
            }
        )*
    };
}

unsafe impl<T: NonZero + ToFromLe> NonZero for Le<T> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroNumValidateError<T>(PhantomData<T>);

macro_rules! impl_nonzero_ints {
    ( $( $t:ident => $inner:ident; )* ) => {
        $(
            impl_tofromle!($t, $inner);

            unsafe impl Persist for Le<$t> {
                type Error = NonZeroNumValidateError<$t>;

                #[inline(always)]
                fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
                    if maybe[..].iter().all(|x| *x == 0) {
                        Err(NonZeroNumValidateError(PhantomData))
                    } else {
                        unsafe { Ok(maybe.assume_valid()) }
                    }
                }

                #[inline(always)]
                fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
                    let buf = unsafe { slice::from_raw_parts(self as *const _ as *const u8,
                                                             mem::size_of::<Self>()) };
                    dst.write_bytes(buf);
                    dst.done()
                }
            }
        )*
    };
}

unsafe impl Persist for NonZeroU8 {
    type Error = NonZeroNumValidateError<NonZeroU8>;

    #[inline(always)]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        if maybe[0] == 0 {
            Err(NonZeroNumValidateError(PhantomData))
        } else {
            unsafe { Ok(maybe.assume_valid()) }
        }
    }

    #[inline(always)]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        dst.write_bytes([self.get()]);
        dst.done()
    }
}

unsafe impl Persist for NonZeroI8 {
    type Error = NonZeroNumValidateError<NonZeroI8>;

    #[inline(always)]
    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error> {
        if maybe[0] == 0 {
            Err(NonZeroNumValidateError(PhantomData))
        } else {
            unsafe { Ok(maybe.assume_valid()) }
        }
    }

    #[inline(always)]
    fn write_canonical<'b>(&self, mut dst: UninitBytes<'b, Self>) -> &'b mut [u8] {
        dst.write_bytes([self.get() as u8]);
        dst.done()
    }
}

macro_rules! impl_tofromle {
    ($t:ident, $inner:ident) => {
        impl ToFromLe for $t {
            #[inline(always)]
            fn to_le(this: Self) -> Self {
                unsafe {
                    let this: $inner = mem::transmute(this);
                    mem::transmute(this.to_le())
                }
            }
            #[inline(always)]
            fn from_le(le_this: Self) -> Self {
                unsafe {
                    let le_this: $inner = mem::transmute(le_this);
                    let this = $inner::from_le(mem::transmute(le_this));
                    mem::transmute(this)
                }
            }
        }

        impl From<Le<$t>> for $t {
            #[inline(always)]
            fn from(le: Le<$t>) -> Self {
                le.get()
            }
        }

        impl cmp::PartialEq for Le<$t> {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                cmp::PartialEq::eq(&self.get(), &other.get())
            }
        }
        impl cmp::PartialEq<$t> for Le<$t> {
            #[inline(always)]
            fn eq(&self, other: &$t) -> bool {
                cmp::PartialEq::eq(&self.get(), other)
            }
        }
        impl cmp::PartialEq<Le<$t>> for $t {
            #[inline(always)]
            fn eq(&self, other: &Le<$t>) -> bool {
                cmp::PartialEq::eq(self, &other.get())
            }
        }
        impl cmp::Eq for Le<$t> {}

        impl cmp::PartialOrd for Le<$t> {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
                cmp::PartialOrd::partial_cmp(&self.get(), &other.get())
            }
        }
        impl cmp::PartialOrd<$t> for Le<$t> {
            #[inline(always)]
            fn partial_cmp(&self, other: &$t) -> Option<cmp::Ordering> {
                cmp::PartialOrd::partial_cmp(&self.get(), other)
            }
        }
        impl cmp::PartialOrd<Le<$t>> for $t {
            #[inline(always)]
            fn partial_cmp(&self, other: &Le<$t>) -> Option<cmp::Ordering> {
                cmp::PartialOrd::partial_cmp(self, &(other.get()))
            }
        }
        impl cmp::Ord for Le<$t> {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> cmp::Ordering {
                cmp::Ord::cmp(&self.get(), &other.get())
            }
        }
    }
}

impl_ints!(
    u16, i16,
    u32, i32,
    u64, i64,
    u128, i128,
);

impl_nonzero_ints!(
    NonZeroU16 =>   u16; NonZeroI16  =>  i16;
    NonZeroU32 =>   u32; NonZeroI32  =>  i32;
    NonZeroU64 =>   u64; NonZeroI64  =>  i64;
    NonZeroU128 => u128; NonZeroI128 =>  i128;
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment() {
        assert_eq!(mem::align_of::<Le<u16>>(),  1);
        assert_eq!(mem::align_of::<Le<u32>>(),  1);
        assert_eq!(mem::align_of::<Le<u64>>(),  1);
        assert_eq!(mem::align_of::<Le<u128>>(), 1);

        assert_eq!(mem::align_of::<Le<i16>>(),  1);
        assert_eq!(mem::align_of::<Le<i32>>(),  1);
        assert_eq!(mem::align_of::<Le<i64>>(),  1);
        assert_eq!(mem::align_of::<Le<i128>>(), 1);
    }

    #[test]
    fn test() {
        assert_eq!(Le::new(0x1234_5678_u32).canonical_bytes(),
                   [0x78, 0x56, 0x34, 0x12]);

        assert_eq!(Le::new(NonZeroU32::new(0x1234_5678_u32).unwrap()).canonical_bytes(),
                   [0x78, 0x56, 0x34, 0x12]);
    }
}
