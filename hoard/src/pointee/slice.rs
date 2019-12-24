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

/// The length of a slice.
#[repr(transparent)]
pub struct SliceLen<T> {
    marker: PhantomData<fn() -> T>,
    len: Le<u64>,
}

impl<T> SliceLen<T> {
    /// Creates a new `SliceLen<T>`.
    #[inline(always)]
    pub fn new(len: usize) -> Option<Self> {
        Self::try_from(len).ok()
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

impl<T> From<SliceLen<T>> for u64 {
    #[inline(always)]
    fn from(len: SliceLen<T>) -> u64 {
        len.len.get()
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

//unsafe impl<T> Sync for SliceLen<T> {}
//unsafe impl<T> Send for SliceLen<T> {}

/// Error when a slice length is too large for a given type.
#[derive(Debug, PartialEq, Eq)]
pub struct SliceLenError(());
impl ValidationError for SliceLenError {}

impl<T> TryFrom<usize> for SliceLen<T> {
    type Error = SliceLenError;

    #[inline]
    fn try_from(len: usize) -> Result<Self, Self::Error> {
        Self::try_from(u64::try_from(len).unwrap())
    }
}

impl<T> TryFrom<u64> for SliceLen<T> {
    type Error = SliceLenError;

    #[inline]
    fn try_from(len: u64) -> Result<Self, Self::Error> {
        let size: u64 = mem::size_of::<T>().try_into().unwrap();

        size.checked_mul(len)
            .and_then(|len| {
                if len <= (isize::max_value() as u64) {
                    Some(Self { marker: PhantomData, len: len.into() })
                } else {
                    None
                }
            }).ok_or(SliceLenError(()))
    }
}

impl<T> TryFrom<Le<u64>> for SliceLen<T> {
    type Error = SliceLenError;

    #[inline]
    fn try_from(len: Le<u64>) -> Result<Self, Self::Error> {
        Self::try_from(u64::from(len))
    }
}

impl<T: Persist> Persist for SliceLen<T> {
    type Persist = SliceLen<T::Persist>;
}

impl<T> ValidateBlob for SliceLen<T> {
    type Error = SliceLenError;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        blob.validate_bytes(|blob| {
            let slice = &blob[..];
            let len = u64::from_le_bytes(slice.try_into().unwrap());

            Self::try_from(len)?;
            Ok(unsafe { blob.assume_valid() })
        })
    }
}

unsafe impl<'a, T: Persist, Z> ValidateChildren<'a, Z> for SliceLen<T> {
    type State = ();
    fn validate_children(_: &'a SliceLen<T::Persist>) -> () {}

    fn poll<V: PtrValidator<Z>>(this: &'a SliceLen<T::Persist>, _: &mut (), _: &V) -> Result<&'a Self, V::Error> {
        assert_eq!(Layout::new::<T>(), Layout::new::<T::Persist>(),
                   "Incorrect Persist implementation on {}; layouts differ", type_name::<T>());
        Ok(unsafe { mem::transmute(this) })
    }
}

impl<Z, T: Persist> Decode<Z> for SliceLen<T> {
}

impl<Z, T: Encoded<Z>> Encoded<Z> for SliceLen<T> {
    type Encoded = SliceLen<T::Encoded>;
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
    fn make_fat_ptr(thin: *const (), len: Self::Metadata) -> *const [T] {
        ptr::slice_from_raw_parts(thin as *const T, len.into())
    }

    #[inline(always)]
    fn make_fat_ptr_mut(thin: *mut (), len: Self::Metadata) -> *mut [T] {
        ptr::slice_from_raw_parts_mut(thin as *mut T, len.into())
    }

    #[inline(always)]
    fn layout(len: Self::Metadata) -> Layout {
        let item_len_rounded_up = (size_of::<T>() + align_of::<T>() - 1) / size_of::<T>() * size_of::<T>();
        let size = item_len_rounded_up * len.get();
        Layout::from_size_align(size, align_of::<T>())
            .unwrap() // optimized away as the above is wrapping arithmetic in release mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        let slice = &[(1u8, 2u64); 100][..];

        let expected_layout = Layout::for_value(slice);
        assert_eq!(expected_layout.size(), 1600);
        assert_eq!(expected_layout.align(), 8);

        let metadata = <[_] as Pointee>::metadata(slice);
        assert_eq!(metadata.len.get(), 100);
        assert_eq!(<[_] as Pointee>::layout(metadata), expected_layout);
    }
}
