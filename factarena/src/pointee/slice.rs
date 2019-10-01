use super::*;

use core::marker::PhantomData;
use core::cmp;
use core::convert::TryFrom;
use core::fmt;
use core::hash;
use core::mem;
use core::ptr;

#[repr(transparent)]
pub struct SliceLen<T> {
    marker: PhantomData<*const T>,

    // FIXME: change this to Le<u64>
    len: usize,
}

impl<T> SliceLen<T> {
    #[inline(always)]
    pub const unsafe fn new_unchecked(len: usize) -> Self {
        Self { marker: PhantomData, len }
    }

    #[inline(always)]
    pub fn get(self) -> usize {
        self.len
    }
}

impl<T> fmt::Debug for SliceLen<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.len.fmt(f)
    }
}

impl<T> Clone for SliceLen<T> {
    #[inline(always)]
    fn clone(&self) -> Self { *self }
}
impl<T> Copy for SliceLen<T> {}

impl<T> cmp::PartialEq for SliceLen<T> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
    }
}
impl<T> cmp::Eq for SliceLen<T> {}

impl<T> cmp::PartialOrd for SliceLen<T> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.len.partial_cmp(&other.len)
    }
}
impl<T> cmp::Ord for SliceLen<T> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.len.cmp(&other.len)
    }
}

impl<T> hash::Hash for SliceLen<T> {
    #[inline(always)]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.len.hash(state)
    }
}


impl<T> From<SliceLen<T>> for usize {
    #[inline(always)]
    fn from(len: SliceLen<T>) -> usize {
        len.len
    }
}

impl<T> From<SliceLen<T>> for Layout {
    #[inline(always)]
    fn from(len: SliceLen<T>) -> Layout {
        match Layout::array::<T>(len.len) {
            Ok(layout) => layout,
            Err(e) => {
                panic!("Layout failed: {:?}", e)
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceLenError(pub usize);

impl<T> TryFrom<usize> for SliceLen<T> {
    type Error = SliceLenError;

    #[inline]
    fn try_from(len: usize) -> Result<Self, Self::Error> {
        match mem::size_of::<T>().checked_mul(len) {
            Some(len_bytes) if len_bytes <= (isize::max_value() as usize) => Ok( unsafe { SliceLen::new_unchecked(len) } ),
            _ => Err(SliceLenError(len)),
        }
    }
}

unsafe impl<T> Metadata for [T] {
    type Metadata = SliceLen<T>;

    #[inline(always)]
    fn ptr_metadata(&self) -> Self::Metadata {
        unsafe {
            SliceLen::new_unchecked(self.len())
        }
    }
}

unsafe impl<T> Pointee for [T] {
    //type Owned = Vec<T>;

    #[inline(always)]
    fn layout(len: Self::Metadata) -> Layout {
        len.into()
    }

    #[inline(always)]
    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const [T] {
        ptr::slice_from_raw_parts(thin as *const T, len.into())
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut [T] {
        ptr::slice_from_raw_parts_mut(thin as *mut T, len.into())
    }
}
