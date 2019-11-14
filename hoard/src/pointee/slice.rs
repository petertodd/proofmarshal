use super::*;

use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::mem;
use core::ptr;

use owned::Take;
use leint::Le;

use crate::{Zone, Ref};
use crate::marshal::{Primitive, blob::*};

/// The length of a slice.
#[repr(transparent)]
pub struct SliceLen<T> {
    marker: PhantomData<*const T>,
    len: Le<u64>,
}

impl<T> SliceLen<T> {
    /// Creates a new `SliceLen<T>`.
    #[inline(always)]
    pub fn new(len: usize) -> Option<Self> {
        mem::size_of::<T>().checked_mul(len)
            .and_then(|len_bytes| {
                if len_bytes <= (isize::max_value() as usize) {
                    let len = len.try_into().ok().unwrap();
                    Some(unsafe { SliceLen::new_unchecked(len) })
                } else {
                    None
                }
            })
    }

    /// Creates a new `SliceLen<T>` without checking that the length is valid.
    #[inline(always)]
    pub unsafe fn new_unchecked(len: usize) -> Self {
        Self {
            marker: PhantomData,
            len: (len as u64).into(),
        }
    }

    /// Gets the underlying length.
    #[inline(always)]
    pub fn get(self) -> usize {
        self.len.get() as usize
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
        len.get()
    }
}

impl<T> From<SliceLen<T>> for Layout {
    #[inline(always)]
    fn from(len: SliceLen<T>) -> Layout {
        match Layout::array::<T>(len.get()) {
            Ok(layout) => layout,
            Err(e) => {
                panic!("Layout failed: {:?}", e)
            },
        }
    }
}

unsafe impl<T> Sync for SliceLen<T> {}
unsafe impl<T> Send for SliceLen<T> {}

/// Error when a slice length is too large for a given type.
#[derive(Debug, PartialEq, Eq)]
pub struct SliceLenError(());

impl<T> TryFrom<usize> for SliceLen<T> {
    type Error = SliceLenError;

    #[inline]
    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::new(len).ok_or(SliceLenError(()))
    }
}

impl<T> Primitive for SliceLen<T> {
    type Error = SliceLenError;

    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<u64>());

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.len.get().to_le_bytes())?
           .finish()
    }

    fn validate_blob<'p,P>(blob: Blob<'p, Self, P>) -> Result<FullyValidBlob<'p, Self, P>, Self::Error> {
        todo!() //Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p,P>(blob: FullyValidBlob<'p, Self, P>) -> Self {
        *<Self as Primitive>::load_blob(blob)
    }

    fn load_blob<'p, P>(blob: FullyValidBlob<'p, Self, P>) -> Ref<'p, Self> {
        todo!() //unsafe { blob.assume_valid_ref() }
    }
}

unsafe impl<T> Persist for SliceLen<T> {}

unsafe impl<T> Pointee for [T] {
    type Metadata = SliceLen<T>;

    #[inline(always)]
    fn metadata_from_dropped(dropped: &MaybeDropped<Self>) -> Self::Metadata {
        unsafe {
            let len = dropped.get_unchecked().len();
            SliceLen::new_unchecked(len)
        }
    }

    #[inline(always)]
    fn align(_: Self::Metadata) -> usize {
        mem::align_of::<T>()
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

unsafe impl<T> PtrSized for [T] {
    #[inline(always)]
    fn size(len: Self::Metadata) -> usize {
        Layout::from(len).size()
    }
}
