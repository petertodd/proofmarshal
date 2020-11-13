use std::fmt;
use std::ptr::{self, NonNull};

use thiserror::Error;

use crate::blob::Blob;

pub trait Pointee {
    type Metadata : 'static + Copy + Blob + fmt::Debug + Eq + Ord;
    type LayoutError : 'static + std::error::Error + Send;

    fn metadata(this: *const Self) -> Self::Metadata;

    fn sized_metadata() -> Self::Metadata
        where Self: Sized
    {
        unreachable!()
    }

    /// Makes a fat pointer from a thin pointer.
    fn make_fat_ptr(thin: *const (), metadata: Self::Metadata) -> *const Self;

    /// Makes a mutable fat pointer from a thin pointer.
    fn make_fat_ptr_mut(thin: *mut (), metadata: Self::Metadata) -> *mut Self;

    /// Makes a fat `NonNull` from a thin `NonNull`.
    #[inline(always)]
    fn make_fat_non_null(thin: NonNull<()>, metadata: Self::Metadata) -> NonNull<Self> {
        let p: *mut Self = Self::make_fat_ptr_mut(thin.as_ptr(), metadata);
        unsafe {
            NonNull::new_unchecked(p)
        }
    }
}

impl<T> Pointee for T {
    type Metadata = ();
    type LayoutError = !;

    fn metadata(_: *const Self) -> Self::Metadata {
        ()
    }

    fn sized_metadata() -> Self::Metadata {
        ()
    }

    fn make_fat_ptr(thin: *const (), _: ()) -> *const Self {
        thin.cast()
    }

    fn make_fat_ptr_mut(thin: *mut (), _: ()) -> *mut Self {
        thin.cast()
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[non_exhaustive]
pub struct SliceLayoutError;

impl<T> Pointee for [T] {
    type Metadata = usize;

    type LayoutError = SliceLayoutError;

    fn metadata(this: *const Self) -> Self::Metadata {
        this.len()
    }

    fn make_fat_ptr(thin: *const (), len: usize) -> *const Self {
        ptr::slice_from_raw_parts(thin as *const T, len)
    }

    fn make_fat_ptr_mut(thin: *mut (), len: usize) -> *mut Self {
        ptr::slice_from_raw_parts_mut(thin as *mut T, len)
    }
}
