use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::slice;

use super::{Blob, BlobDyn, MaybeValid};

pub struct Bytes<'a, T: ?Sized + BlobDyn> {
    marker: PhantomData<&'a [u8]>,
    ptr: *const T,
}

pub struct ValidBytes<'a, T: ?Sized + BlobDyn>(Bytes<'a, T>);

impl<'a, T: ?Sized + BlobDyn> Clone for Bytes<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized + BlobDyn> Copy for Bytes<'a, T> {}

impl<'a, T: ?Sized + BlobDyn> Clone for ValidBytes<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized + BlobDyn> Copy for ValidBytes<'a, T> {}

/// Uninitialized `Bytes`
pub struct BytesUninit<'a, T: ?Sized + BlobDyn> {
    marker: PhantomData<fn(&'a mut [u8]) -> &'a [u8]>,
    ptr: *mut T,
}

pub struct StructCursor<'a, T: ?Sized + BlobDyn> {
    bytes: Bytes<'a, T>,
    idx: usize,
}

// ------ Deref impls -----
impl<'a, T: ?Sized + BlobDyn> Deref for Bytes<'a, T> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        let size = T::try_size(T::metadata(self.ptr)).ok().expect("metadata to be correct");

        unsafe {
            slice::from_raw_parts(self.ptr.cast(), size)
        }
    }
}

impl<'a, T: ?Sized + BlobDyn> Deref for ValidBytes<'a, T> {
    type Target = Bytes<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: ?Sized + BlobDyn> Deref for BytesUninit<'a, T> {
    type Target = [MaybeUninit<u8>];

    fn deref(&self) -> &Self::Target {
        let size = T::try_size(T::metadata(self.ptr)).ok().expect("metadata to be correct");

        unsafe {
            slice::from_raw_parts(self.ptr.cast(), size)
        }
    }
}

impl<'a, T: ?Sized + BlobDyn> DerefMut for BytesUninit<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let size = T::try_size(T::metadata(self.ptr)).ok().expect("metadata to be correct");

        unsafe {
            slice::from_raw_parts_mut(self.ptr.cast(), size)
        }
    }
}

impl<T: ?Sized + BlobDyn> fmt::Debug for Bytes<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Bytes")
            .field(&&self[..])
            .finish()
    }
}

impl<T: ?Sized + BlobDyn> fmt::Debug for ValidBytes<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ValidBytes")
            .field(&&self[..])
            .finish()
    }
}


impl<'a, T: ?Sized + BlobDyn> Bytes<'a, T> {
    pub unsafe fn new_unchecked(ptr: *const u8, metadata: T::Metadata) -> Self {
        let ptr = T::make_fat_ptr(ptr as *const (), metadata);
        Self {
            marker: PhantomData,
            ptr,
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        T::metadata(self.ptr)
    }

    pub fn struct_fields(self) -> StructCursor<'a, T> {
        StructCursor {
            bytes: self,
            idx: 0,
        }
    }

    pub fn assume_valid(self) -> ValidBytes<'a, T> {
        ValidBytes(self)
    }
}

impl<'a, T: ?Sized + BlobDyn> StructCursor<'a, T> {
    pub fn decode_field<F: Blob>(&mut self) -> Result<MaybeValid<F>, F::DecodeBytesError> {
        let field_bytes = self.bytes.get(self.idx .. self.idx + F::SIZE)
                                        .expect("overflow");
        let field_bytes = Bytes::<F>::try_from(field_bytes).unwrap();
        let field = F::decode_bytes(field_bytes)?;
        self.idx += F::SIZE;
        Ok(field)
    }

    pub fn trust_field<F: Blob>(&mut self) -> Result<F, F::DecodeBytesError> {
        self.decode_field()
            .map(|maybe| maybe.trust())
    }

    #[track_caller]
    pub fn assert_done(self) -> Bytes<'a, T> {
        assert_eq!(self.idx, self.bytes.len(), "not all bytes used");
        self.bytes
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct TryFromSliceError;

impl<'a, T: Blob> TryFrom<&'a [u8]> for Bytes<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [u8]) -> Result<Self, TryFromSliceError> {
        if slice.len() == T::SIZE {
            Ok(unsafe {
                Self::new_unchecked(slice.as_ptr().cast(), ())
            })
        } else {
            Err(TryFromSliceError)
        }
    }
}



impl<'a, T: Blob> TryFrom<&'a mut [u8]> for BytesUninit<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a mut [u8]) -> Result<Self, TryFromSliceError> {
        if slice.len() == T::SIZE {
            Ok(unsafe {
                Self::new_unchecked(slice.as_mut_ptr().cast())
            })
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<'a, T: Blob> TryFrom<&'a mut [MaybeUninit<u8>]> for BytesUninit<'a, T> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a mut [MaybeUninit<u8>]) -> Result<Self, TryFromSliceError> {
        if slice.len() == T::SIZE {
            Ok(unsafe {
                Self::new_unchecked(slice.as_mut_ptr().cast())
            })
        } else {
            Err(TryFromSliceError)
        }
    }
}

impl<'a, T: ?Sized + BlobDyn> BytesUninit<'a, T> {
    pub fn from_bytes(slice: &'a mut [u8], metadata: T::Metadata) -> Result<Self, T::LayoutError> {
        let blob_size = T::try_size(metadata)?;
        assert_eq!(slice.len(), blob_size);

        let ptr = T::make_fat_ptr_mut(slice.as_mut_ptr().cast(), metadata);
        unsafe { Ok(Self::new_unchecked(ptr)) }
    }

    pub unsafe fn new_unchecked(ptr: *mut T) -> Self {
        Self {
            marker: PhantomData,
            ptr,
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        T::metadata(self.ptr)
    }


    /// Initializes with a slice.
    #[track_caller]
    pub fn write_bytes(mut self, src: &[u8]) -> Bytes<'a, T> {
        assert_eq!(self.len(), src.len(), "length mismatch");

        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                self.as_mut_ptr().cast::<u8>(),
                self.len()
            );

            Bytes {
                marker: PhantomData,
                ptr: self.ptr,
            }
        }
    }

    pub fn write_struct(self) -> WriteStruct<'a, T> {
        WriteStruct {
            bytes: self,
            written: 0,
        }
    }
}

pub struct WriteStruct<'a, T: ?Sized + BlobDyn> {
    bytes: BytesUninit<'a, T>,
    written: usize,
}

impl<'a, T: ?Sized + BlobDyn> WriteStruct<'a, T> {
    #[track_caller]
    pub fn write_field<F: Blob>(mut self, blob: &F) -> Self {
        let field_bytes = self.bytes.get_mut(self.written .. self.written + F::SIZE)
                                    .expect("overflow");
        let field_bytes = BytesUninit::<F>::try_from(field_bytes).unwrap();

        blob.encode_bytes(field_bytes);
        self.written += F::SIZE;
        self
    }

    #[track_caller]
    pub fn done(self) -> Bytes<'a, T> {
        assert_eq!(self.bytes.len(), self.written, "not all bytes written");

        Bytes {
            marker: PhantomData,
            ptr: self.bytes.ptr,
        }
    }
}
