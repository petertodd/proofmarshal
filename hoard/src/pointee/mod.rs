use std::fmt;
use std::ptr::NonNull;

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
