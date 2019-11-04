//! In-place persistence.

#![feature(never_type)]

use core::any::type_name;
use core::convert::TryFrom;
use core::ptr::NonNull;
use core::marker::PhantomData;
use core::ops;
use core::slice;

use std::io::{self, Write};

use pointee::{DynSized, Pointee};

pub mod errors;
use self::errors::*;

mod scalars;
mod leint;
pub use self::leint::Le;

pub trait Persist {
    fn write_canonical<W: Write>(&self, dst: W) -> io::Result<W>;

    fn canonical_bytes(&self) -> Box<[u8]> {
        self.write_canonical(vec![]).unwrap().into_boxed_slice()
    }
}

pub trait Validate<V: ?Sized = ()> : Persist + DynSized {
    type Error;

    /// Validate with a validator.
    fn validate<'a>(maybe: MaybeValid<'a, Self>, validator: &mut V) -> Result<Valid<'a, Self>, Self::Error>;
}

#[derive(Debug)]
pub struct MaybeValid<'a, T: ?Sized + Pointee> {
    marker: PhantomData<&'a T>,
    thin: NonNull<()>,
    metadata: T::Metadata,
}

impl<'a, T: ?Sized + DynSized> MaybeValid<'a, T> {
    pub fn from_slice(slice: &'a [u8], metadata: T::Metadata) -> Result<Self, MaybeValidFromSliceError> {
        assert_eq!(T::align(metadata), 1,
                "{} requires alignment", type_name::<T>());

        if slice.len() == T::size(metadata) {
            Ok(Self {
                marker: PhantomData,
                thin: NonNull::new(slice.as_ptr() as *mut ()).unwrap(),
                metadata,
            })
        } else {
            Err(MaybeValidFromSliceError(()))
        }
    }
}

impl<'a, T: ?Sized + Pointee> MaybeValid<'a, T> {
    #[inline(always)]
    pub unsafe fn assume_valid(self) -> Valid<'a, T> {
        Valid {
            marker: self.marker,
            thin: self.thin,
            metadata: self.metadata,
        }
    }
}

impl<'a, T: ?Sized + DynSized> ops::Deref for MaybeValid<'a, T> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.thin.cast().as_ptr(),
                                  T::size(self.metadata))
        }
    }
}

impl<'a, T> TryFrom<&'a [u8]> for MaybeValid<'a, T> {
    type Error = MaybeValidFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        MaybeValid::from_slice(slice, T::make_sized_metadata())
    }
}


#[derive(Debug)]
pub struct Valid<'a, T: ?Sized + Pointee> {
    marker: PhantomData<&'a T>,
    thin: NonNull<()>,
    metadata: T::Metadata,
}

impl<'a, T: ?Sized + Pointee> ops::Deref for Valid<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe {
            let fat: *const T = T::make_fat_non_null(self.thin, self.metadata).as_ptr();
            &*fat
        }
    }
}
