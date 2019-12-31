//! Targets of pointers.

use std::alloc::Layout;
use std::any::Any;
use std::convert::TryInto;
use std::fmt;
use std::hash::Hash;
use std::mem::{self, MaybeUninit};
use std::ptr::{self, NonNull};

use leint::Le;

mod maybedropped;
pub use self::maybedropped::MaybeDropped;

pub trait Metadata : 'static + crate::marshal::Primitive + fmt::Debug + Send + Sync {
    fn kind(&self) -> MetadataKind;
}

#[derive(Debug)]
pub enum MetadataKind {
    Sized,
    Len(u64),
}

impl Metadata for () {
    #[inline(always)]
    fn kind(&self) -> MetadataKind {
        MetadataKind::Sized
    }
}

/// A target of a pointer.
///
/// # Safety
///
/// Other code can assume `Pointee` is implemented correctly.
pub unsafe trait Pointee {
    /// Fat pointer metadata.
    type Metadata : 'static + Metadata + Copy + Eq + Ord + Hash + Send + Sync;
    type LayoutError : 'static + std::error::Error + Send + Sync;

    fn try_layout(metadata: Self::Metadata) -> Result<Layout, Self::LayoutError>;

    fn metadata(this: &Self) -> Self::Metadata {
        Self::metadata_from_dropped(MaybeDropped::from_ref(this))
    }

    fn metadata_from_dropped(dropped: &MaybeDropped<Self>) -> Self::Metadata;

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

    fn try_layout(_: ()) -> Result<Layout, !> {
        Ok(Layout::new::<T>())
    }

    fn make_sized_metadata() -> Self::Metadata {
        unsafe {
            MaybeUninit::uninit().assume_init()
        }
    }

    fn metadata_from_dropped(_: &MaybeDropped<Self>) -> () {}

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), _: Self::Metadata) -> *const Self {
        thin as *const Self
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), _: Self::Metadata) -> *mut Self {
        thin as *mut Self
    }
}

/*
unsafe impl<T> Pointee for [T] {
    type Metadata = Le<u64>;

    #[inline(always)]
    fn metadata_from_dropped(dropped: &MaybeDropped<Self>) -> Self::Metadata {
        let len: u64 = unsafe {
            dropped.get_unchecked().len()
        }.try_into().unwrap();
        len.into()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), len: Le<u64>) -> *const [T] {
        ptr::slice_from_raw_parts(
            thin as *const T,
            len.get().try_into().unwrap()
        )
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut [T] {
        ptr::slice_from_raw_parts_mut(
            thin as *mut T,
            len.get().try_into().unwrap()
        )
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
    }
}
