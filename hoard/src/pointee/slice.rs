use super::*;

use core::any::type_name;
use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem::{self, size_of, align_of};
use core::ptr;

use owned::Take;
use leint::Le;

use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::marshal::Primitive;

/// Error when a slice length is too large for a given type.
#[derive(Debug, PartialEq, Eq)]
pub struct SliceLenError(());
impl ValidationError for SliceLenError {}

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

    #[inline(always)]
    fn layout(len: Self::Metadata) -> Layout {
        /*
        let item_len_rounded_up = (size_of::<T>() + align_of::<T>() - 1) / size_of::<T>() * size_of::<T>();
        let size = item_len_rounded_up * len.get();
        Layout::from_size_align(size, align_of::<T>())
            .unwrap() // optimized away as the above is wrapping arithmetic in release mode
        */ todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
    }
}
