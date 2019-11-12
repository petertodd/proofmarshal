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
use crate::marshal::{*, blob::*};

/// The length of a slice.
#[repr(transparent)]
pub struct SliceLen<T> {
    marker: PhantomData<*const T>,

    // FIXME: change this to Le<u64>
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

impl<T, Z: Zone> Save<Z> for SliceLen<T> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<u64>());

    type SavePoll = Self;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        this.take_sized()
    }
}

impl<T, Z: Zone> SavePoll<Z> for SliceLen<T> {
    type Target = Self;
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&self.len.get().to_le_bytes())?
           .done()
    }
}

impl<T, Z: Zone> Load<Z> for SliceLen<T> {
    type Error = !;

    type ValidateChildren = ();

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self {
        *Self::load_blob(blob, loader)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, _: &impl Loader<Z>) -> Ref<'p, Self> {
        unsafe { blob.assume_valid_ref() }
    }
}


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
