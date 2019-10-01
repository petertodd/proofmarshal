use core::alloc::Layout;
use core::borrow::Borrow;
use core::fmt;
use core::mem;
use core::ptr::NonNull;

pub trait Metadata {
    /// Metadata about this type.
    type Metadata : Copy + fmt::Debug;

    fn metadata(this: &Self) -> Self::Metadata;
}

/// The target of a pointer.
pub trait Type : Metadata {
    /// The owned form of this type.
    type Owned : Borrow<Self>;

    /// Makes a pointer to value of this type.
    fn make_fat_ptr(thin: *const (), metadata: Self::Metadata) -> *const Self;

    /// Makes a mutable pointer to a value of this type.
    fn make_fat_ptr_mut(thin: *mut (), metadata: Self::Metadata) -> *mut Self;

    /// Makes a `NonNull` to a value of this type.
    #[inline(always)]
    fn make_fat_non_null(thin: NonNull<()>, metadata: Self::Metadata) -> NonNull<Self> {
        let p: *mut Self = Self::make_fat_ptr_mut(thin.as_ptr(), metadata);
        unsafe {
            NonNull::new_unchecked(p)
        }
    }

    /// Creates a `Layout` from this type's metadata.
    fn layout(metadata: Self::Metadata) -> Layout;

    /// Create the metadata of a sized type.
    fn make_sized_metadata() -> Self::Metadata
        where Self: Sized,
    {
        unsafe {
            assert_eq!(mem::size_of::<Self::Metadata>(), 0);
            mem::MaybeUninit::uninit().assume_init()
        }
    }
}

impl<T> Metadata for T {
    type Metadata = ();

    fn metadata(_: &Self) -> () {}
}

impl<T: Metadata> Type for T {
    type Owned = T;

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), _: Self::Metadata) -> *const Self {
        thin as *const Self
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), _: Self::Metadata) -> *mut Self {
        thin as *mut Self
    }

    /// The in-memory layout of sized types ignores type metadata.
    #[inline(always)]
    fn layout(_: Self::Metadata) -> Layout {
        Layout::new::<Self>()
    }
}

pub trait CoercePtr<Q> {
    type Coerced : ?Sized;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
