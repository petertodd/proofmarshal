use core::cmp;
use core::fmt::{self, Display, Debug};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem;
use core::num::{
    NonZeroU8,   NonZeroI8,
    NonZeroU16,  NonZeroI16,
    NonZeroU32,  NonZeroI32,
    NonZeroU64,  NonZeroI64,
    NonZeroU128, NonZeroI128,
};
use core::slice;

use crate::util::nonzero::NonZero;

use super::{Arena, Persist, Unverified, VerifyPtr};

#[repr(packed)]
pub struct Le<T: ToFromLe>(T);

pub trait ToFromLe
    : 'static + Copy + PartialEq + Eq + PartialOrd + Ord + Display + Debug
{
    fn to_le(this: Self) -> Self;
    fn from_le(le_this: Self) -> Self;
}

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

impl<T: ToFromLe> Debug for Le<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Le({:?})", self.get())
    }
}
impl<T: ToFromLe> Display for Le<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
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

unsafe impl<T: NonZero + ToFromLe> NonZero for Le<T> {}

macro_rules! impl_ints {
    ( $( $t:ident, )* ) => {
        $(
            impl_tofromle!($t, $t);

            unsafe impl<A: Arena> Persist<A> for Le<$t> {
                type Error = !;

                #[inline]
                fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
                    unsafe { Ok(unver.assume_init()) }
                }
            }
        )*
    };
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateNonZeroError<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> Default for ValidateNonZeroError<T> {
    #[inline]
    fn default() -> Self {
        ValidateNonZeroError(PhantomData)
    }
}

unsafe impl<A: Arena> Persist<A> for NonZeroU8 {
    type Error = ValidateNonZeroError<NonZeroU8>;

    #[inline]
    fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        if unver[0] == 0 {
            Err(ValidateNonZeroError::default())
        } else {
            unsafe { Ok(unver.assume_init()) }
        }
    }
}

unsafe impl<A: Arena> Persist<A> for NonZeroI8 {
    type Error = ValidateNonZeroError<NonZeroI8>;

    #[inline]
    fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        if unver[0] == 0 {
            Err(ValidateNonZeroError::default())
        } else {
            unsafe { Ok(unver.assume_init()) }
        }
    }
}

macro_rules! impl_nonzero_ints {
    ( $( $t:ident => $inner:ident; )* ) => {
        $(
            impl_tofromle!($t, $inner);

            unsafe impl<A: Arena> Persist<A> for Le<$t> {
                type Error = ValidateNonZeroError<Le<$t>>;

                #[inline]
                fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
                    if unver.iter().all(|x| *x == 0) {
                        Err(ValidateNonZeroError::default())
                    } else {
                        unsafe { Ok(unver.assume_init()) }
                    }
                }
            }
        )*
    };
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
    fn nonzero() {
        let unver = Unverified::<Le<NonZeroU32>>::new(&[0,0,0,0]);
        let r = <Le<NonZeroU32> as Persist>::verify(unver, &()).unwrap_err();

        assert_eq!(r, ValidateNonZeroError::<Le<NonZeroU32>>::default());

        let valid = [0x12,0x34,0x56,0x78];
        let unver = Unverified::<Le<NonZeroU32>>::new(&valid);
        let r = <Le<NonZeroU32> as Persist>::verify(unver, &()).unwrap();

        let expected = NonZeroU32::new(0x78_56_34_12).unwrap();
        assert_eq!(*r, Le::new(expected));
    }
}
