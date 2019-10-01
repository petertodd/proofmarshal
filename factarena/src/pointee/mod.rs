/// Targets of pointers.

use core::fmt;
use core::hash::Hash;
use core::ptr::NonNull;
use core::mem::MaybeUninit;

use core::alloc::Layout;

pub mod slice;
pub mod emplace;

/// The pointer metadata of a type.
pub unsafe trait Metadata {
    type Metadata : Sized + Copy + fmt::Debug + Eq + Ord + Hash;

    fn ptr_metadata(&self) -> Self::Metadata;

    fn make_sized_metadata() -> Self::Metadata
        where Self: Sized
    {
        unreachable!()
    }
}

/// Sized types always have no metadata.
unsafe impl<T> Metadata for T {
    type Metadata = ();

    fn ptr_metadata(&self) -> Self::Metadata {
        Self::make_sized_metadata()
    }

    fn make_sized_metadata() -> Self::Metadata {
        unsafe {
            MaybeUninit::uninit().assume_init()
        }
    }
}

/// A target of a pointer.
pub unsafe trait Pointee : Metadata {
    /*
    type Owned : Sized + Borrow<Self>;

    fn from_owned(_owned: Self::Owned) -> Self
        where Self: Sized
    {
        unimplemented!()
    }

    fn to_owned(self) -> Self::Owned
        where Self: Sized
    {
        unimplemented!()
    }
    */

    fn make_fat_ptr(thin: *const (), metadata: Self::Metadata) -> *const Self;
    fn make_fat_ptr_mut(thin: *mut (), metadata: Self::Metadata) -> *mut Self;

    #[inline(always)]
    fn make_fat_non_null(thin: NonNull<()>, metadata: Self::Metadata) -> NonNull<Self> {
        let p: *mut Self = Self::make_fat_ptr_mut(thin.as_ptr(), metadata);
        unsafe {
            NonNull::new_unchecked(p)
        }
    }

    fn layout(metadata: Self::Metadata) -> Layout;
}

unsafe impl<T> Pointee for T {
    /*
    type Owned = T;

    #[inline(always)]
    fn from_owned(owned: Self::Owned) -> Self where Self: Sized {
        owned
    }

    #[inline(always)]
    fn to_owned(self) -> Self::Owned where Self: Sized {
        self
    }
    */

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), _: Self::Metadata) -> *const Self {
        thin as *const Self
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), _: Self::Metadata) -> *mut Self {
        thin as *mut Self
    }

    fn layout(_: Self::Metadata) -> Layout {
        Layout::new::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_metadata() {
        let _:() = ().ptr_metadata();
    }
}
