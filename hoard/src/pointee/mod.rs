//! Targets of pointers.

use std::hash::Hash;
use std::ptr::{self, NonNull};
use std::fmt;

use crate::scalar::Scalar;

use thiserror::Error;
use leint::Le;

/// A target of a pointer.
///
/// # Safety
///
/// Other code can assume `Pointee` is implemented correctly.
pub unsafe trait Pointee {
    /// The metadata associated with pointers to this type.
    type Metadata : 'static + Scalar + Eq + Ord + Hash + Send + Sync + fmt::Debug;

    type LayoutError : 'static + std::error::Error + Send + Sync;

    /*
    fn try_layout(metadata: Self::Metadata) -> Result<Layout, Self::LayoutError>;
    */

    fn metadata(this: &Self) -> Self::Metadata;

    /// Makes the metadata for a sized type.
    ///
    /// Sized types have no metadata, so this is always possible.
    fn make_sized_metadata() -> Self::Metadata
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

unsafe impl<T> Pointee for T {
    type Metadata = ();
    type LayoutError = !;

    fn metadata(_this: &Self) -> Self::Metadata {
        ()
    }

    fn make_sized_metadata() -> Self::Metadata {
    }

    /*
    type LayoutError = !;

    fn try_layout(_: ()) -> Result<Layout, !> {
        Ok(Layout::new::<T>())
    }

    fn make_sized_metadata() -> Self::Metadata {
        unsafe {
            MaybeUninit::uninit().assume_init()
        }
    }

    fn metadata_from_dropped(_: &MaybeDropped<Self>) -> () {}
    */

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), _: Self::Metadata) -> *const Self {
        thin as *const Self
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), _: Self::Metadata) -> *mut Self {
        thin as *mut Self
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct LayoutSliceError;

unsafe impl<T> Pointee for [T] {
    type Metadata = Le<u64>;
    type LayoutError = LayoutSliceError;

    fn metadata(_this: &Self) -> Self::Metadata {
        todo!()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), _: Self::Metadata) -> *const Self {
        todo!()
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), _: Self::Metadata) -> *mut Self {
        todo!()
    }

    /*
    fn try_layout(len: Self::Metadata) -> Result<Layout, Self::LayoutError> {
        let item_size = cmp::max(mem::size_of::<T>(), mem::align_of::<T>());
        item_size.checked_mul(len.get() as usize)
                 .and_then(|size| Layout::from_size_align(size, mem::align_of::<T>()).ok())
                 .ok_or(LayoutSliceError)
    }

    fn metadata_from_dropped(this: &MaybeDropped<Self>) -> Le<u64> {
        (this.as_ptr().len() as u64).into()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const Self {
        ptr::slice_from_raw_parts(thin as *const T, len.get() as usize)
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut Self {
        ptr::slice_from_raw_parts_mut(thin as *mut T, len.get() as usize)
    }
    */
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
    }
}
*/
