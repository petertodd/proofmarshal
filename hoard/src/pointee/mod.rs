//! Targets of pointers.

use core::any::Any;
use core::fmt;
use core::hash::Hash;
use core::ptr::NonNull;
use core::mem::{self, MaybeUninit};

use core::alloc::Layout;

mod maybedropped;
pub use self::maybedropped::MaybeDropped;

pub mod slice;

use crate::load::Validate;

/// A target of a pointer.
///
/// # Safety
///
/// Other code can assume `Pointee` is implemented correctly.
pub unsafe trait Pointee {
    /// Fat pointer metadata.
    type Metadata : Validate + Copy + fmt::Debug + Eq + Ord + Hash + Send + Sync;

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

    /// Computes a `Layout` from the pointer metadata.
    fn layout(metadata: Self::Metadata) -> Layout;

    fn size(metadata: Self::Metadata) -> usize {
        Self::layout(metadata).size()
    }
}

unsafe impl<T> Pointee for T {
    type Metadata = ();

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

    fn layout(_: ()) -> Layout {
        Layout::new::<Self>()
    }
}
