#![allow(incomplete_features)]
#![feature(const_generics)]

#![feature(never_type)]

use std::any::TypeId;

mod array_impls;

pub unsafe trait Static {
    type Static : 'static;
}

pub unsafe trait AnyRef<'a> : 'a {
    fn type_id() -> TypeId
        where Self: Sized;

    fn anyref_type_id(&self) -> TypeId;
}

impl<'a> dyn AnyRef<'a> {
    pub fn is<T: AnyRef<'a>>(&self) -> bool {
        self.anyref_type_id() == T::type_id()
    }

    pub fn downcast_ref<T: AnyRef<'a>>(&'a self) -> Option<&'a T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const _ as *const T)) }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: AnyRef<'a>>(&'a mut self) -> Option<&'a mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut _ as *mut T)) }
        } else {
            None
        }
    }
}

unsafe impl<T: Static> Static for &'_ T {
    type Static = &'static T::Static;
}

unsafe impl<'a, T: Static + AnyRef<'a>> AnyRef<'a> for &'a T {
    fn type_id() -> TypeId
        where Self: Sized
    {
        TypeId::of::<<Self as Static>::Static>()
    }

    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}

unsafe impl<T: Static> Static for &'_ mut T {
    type Static = &'static mut T::Static;
}

unsafe impl<'a, T: Static + AnyRef<'a>> AnyRef<'a> for &'a mut T {
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}

unsafe impl Static for &'_ str {
    type Static = &'static str;
}

unsafe impl<'a> AnyRef<'a> for &'a str {
    #[inline(always)]
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    #[inline(always)]
    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}

unsafe impl Static for &'_ mut str {
    type Static = &'static str;
}

unsafe impl<'a> AnyRef<'a> for &'a mut str {
    #[inline(always)]
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    #[inline(always)]
    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}

unsafe impl<'a, T: Static> Static for *const T {
    type Static = *const T::Static;
}

unsafe impl<'a, T: Static + AnyRef<'a>> AnyRef<'a> for *const T {
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}

unsafe impl<'a, T: Static> Static for *mut T {
    type Static = *mut T::Static;
}

unsafe impl<'a, T: Static + AnyRef<'a>> AnyRef<'a> for *mut T {
    fn type_id() -> TypeId {
        TypeId::of::<<Self as Static>::Static>()
    }

    fn anyref_type_id(&self) -> TypeId {
        Self::type_id()
    }
}


#[macro_export]
macro_rules! impl_anyref_for_static {
    ($($t:ty,)+) => {$(
        unsafe impl $crate::Static for $t {
            type Static = Self;
        }

        unsafe impl $crate::AnyRef<'_> for $t {
            #[inline(always)]
            fn type_id() -> TypeId {
                ::core::any::TypeId::of::<Self>()
            }

            #[inline(always)]
            fn anyref_type_id(&self) -> ::core::any::TypeId {
                Self::type_id()
            }
        }
    )+}
}

impl_anyref_for_static! {
    !,
    (), bool, char,
    f32, f64,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    String,
    TypeId,
}

#[macro_export]
macro_rules! impl_anyref_for_generic {
    ($( $name:ident<$t:ident>, )+) => {$(
        unsafe impl<$t: $crate::Static> $crate::Static for $name<$t> {
            type Static = $name<$t::Static>;
        }

        unsafe impl<'__a, $t: $crate::Static + $crate::AnyRef<'__a>> $crate::AnyRef<'__a> for $name<$t> {
            #[inline(always)]
            fn type_id() -> TypeId {
                ::core::any::TypeId::of::<<Self as $crate::Static>::Static>()
            }

            #[inline(always)]
            fn anyref_type_id(&self) -> ::core::any::TypeId {
                Self::type_id()
            }
        }
    )+}
}

use std::rc::Rc;
use std::rc::Weak as RcWeak;
use std::sync::Arc;
use std::sync::Weak as ArcWeak;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::marker::PhantomData;
use std::num::Wrapping;
use std::cell::{Cell, UnsafeCell};
use std::ptr::NonNull;
use std::pin::Pin;

impl_anyref_for_generic! {
    PhantomData<T>, Option<T>,
    Box<T>, Vec<T>,
    Rc<T>, RcWeak<T>, Arc<T>, ArcWeak<T>,
    ManuallyDrop<T>, MaybeUninit<T>,
    Cell<T>, UnsafeCell<T>,
    NonNull<T>,
    Pin<T>,
    Wrapping<T>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ptr;

    #[test]
    fn downcast_ref_u8() {
        let n: u8 = 1;
        let r: &u8 = &n;
        let any: &dyn AnyRef = r;

        let r2: &u8 = any.downcast_ref().unwrap();
        assert!(ptr::eq(r, r2));

        assert!(any.downcast_ref::<()>().is_none());
        assert!(any.downcast_ref::<&u8>().is_none());
        assert!(any.downcast_ref::<&&u8>().is_none());
    }

    #[test]
    fn downcast_mut_u8() {
        let mut n: u8 = 1;
        let r: &mut u8 = &mut n;
        let r_ptr: *mut u8 = r;
        let any: &mut dyn AnyRef = r;

        let r2: &mut u8 = any.downcast_mut().unwrap();
        assert!(ptr::eq(r_ptr, r2));
        *r2 = 2;
    }

    #[test]
    fn ref_ref() {
        let n: u8 = 1;
        let r1: &u8 = &n;
        let r2: &&u8 = &r1;

        let any: &dyn AnyRef = r2;

        let r3: &&u8 = any.downcast_ref().unwrap();
        assert!(ptr::eq(r2, r3));
    }
}
