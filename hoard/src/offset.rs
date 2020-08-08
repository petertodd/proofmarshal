use std::alloc::{GlobalAlloc, System, Layout};
use std::borrow::Borrow;
use std::cmp;
use std::convert::TryInto;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::num::NonZeroU64;
use std::ptr::NonNull;
use std::hint::unreachable_unchecked;

use thiserror::Error;
use leint::Le;

use owned::{IntoOwned, Take};

use crate::pointee::Pointee;
use crate::refs::Ref;
use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::scalar::*;
use crate::ptr::*;
use crate::pile::*;
use crate::heap::*;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Offset<'pile, 'version> {
    marker: PhantomData<(
                fn(&'pile ()) -> &'pile (),
                &'version (),
            )>,
    raw: Le<NonZeroU64>,
}

unsafe impl Persist for Offset<'_, '_> {}

impl fmt::Debug for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.get().fmt(f)
    }
}

#[derive(Debug, Error)]
#[error("invalid offset")]
#[non_exhaustive]
pub struct ValidateOffsetBlobError;

impl Scalar for Offset<'_, '_> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type ScalarBlobError = ValidateOffsetBlobError;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::ScalarBlobError> {
        let raw = u64::from_le_bytes(blob.as_bytes().try_into().unwrap());

        if raw & 0b1 == 0b1 && (raw >> 1) < Offset::MAX as u64 {
            unsafe { Ok(blob.assume_valid()) }
        } else {
            Err(ValidateOffsetBlobError)
        }
    }

    fn decode_blob<'a>(blob: ValidBlob<'a, Self>) -> Self {
        blob.as_value().clone()
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Ok(blob.as_value())
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}

/*
impl AsPtrImpl<Self> for Offset<'_, '_> {
    fn as_ptr_impl(this: &Self) -> &Self {
        this
    }
}

impl<'p, 'v> PersistPtr for Offset<'p, 'v> {
    type Zone = !;
    type BlobZone = Pile<'p, 'v>;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct OffsetMut<'p, 'v, A = System> {
    marker: PhantomData<A>,
    inner: Offset<'p, 'v>,
}

unsafe impl Persist for OffsetMut<'_, '_> {}

impl fmt::Debug for OffsetMut<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

unsafe impl ValidateBlob for OffsetMut<'_, '_> {
    type BlobError = ValidateOffsetBlobError;

    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(BlobLayout::new_nonzero(mem::size_of::<Self>()))
    }

    fn validate_blob<'a>(blob: Blob<'a, Self>, ignore_padding: bool) -> Result<ValidBlob<'a, Self>, Self::BlobError> {
        let mut fields = blob.validate_fields(ignore_padding);
        fields.validate_blob::<Offset>()?;
        unsafe { Ok(fields.finish()) }
    }
}

impl Load for OffsetMut<'_, '_> {
    type Ptr = !;

    fn decode_blob(blob: ValidBlob<Self>, _: &<Self::Ptr as Ptr>::BlobZone) -> Self {
        blob.as_value().clone()
    }

    fn try_deref_blob<'a>(blob: ValidBlob<'a, Self>, _: &()) -> Result<&'a Self, ValidBlob<'a, Self>> {
        Ok(blob.as_value())
    }
}

impl AsPtrImpl<Self> for OffsetMut<'_, '_> {
    fn as_ptr_impl(this: &Self) -> &Self {
        this
    }
}


/*
impl<'p, 'v, A> Borrow<OffsetMut<'p, 'v, A>> for Offset<'p, 'v> {
    #[inline(always)]
    fn borrow(&self) -> &OffsetMut<'p, 'v, A> {
        self.as_ref()
    }
}

impl<'p, 'v, A> AsRef<OffsetMut<'p, 'v, A>> for Offset<'p, 'v> {
    #[inline(always)]
    fn as_ref(&self) -> &OffsetMut<'p, 'v, A> {
        // SAFETY: #[repr(transparent)]
        unsafe { &*(self as *const Self as *const _) }
    }
}
*/

impl<'p, 'v> From<Offset<'p, 'v>> for usize {
    fn from(offset: Offset<'p, 'v>) -> usize {
        offset.get()
    }
}

impl<'p, 'v> From<Offset<'p, 'v>> for OffsetMut<'p, 'v> {
    fn from(inner: Offset<'p, 'v>) -> Self {
        Self {
            marker: PhantomData,
            inner,
        }
    }
}

impl cmp::PartialEq<usize> for Offset<'_, '_> {
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl cmp::PartialEq<Offset<'_, '_>> for usize {
    fn eq(&self, other: &Offset<'_, '_>) -> bool {
        *self == other.get()
    }
}

*/

impl<'p, 'v> Offset<'p, 'v> {
    /// The largest `Offset`.
    pub const MAX: usize = (1 << 62) - 1;

    /// Creates a new `Offset`.
    ///
    /// Returns `None` if the offset is out of range:
    ///
    /// ```
    /// use hoard::offset::Offset;
    ///
    /// assert!(Offset::new(Offset::MAX + 1)
    ///                .is_none());
    /// ```
    ///
    /// # Examples
    ///
    /// Zero is a valid offset:
    ///
    /// ```
    /// use hoard::offset::Offset;
    ///
    /// Offset::new(0).unwrap();
    /// ```
    pub fn new(offset: usize) -> Option<Self> {
        if offset <= Self::MAX {
            let offset = offset as u64;
            Some(offset.checked_shl(1).map(|offset|
                Self {
                    marker: PhantomData,
                    raw: NonZeroU64::new(offset | 1).unwrap().into(),
                }
            ).unwrap())
        } else {
            None
        }
    }

    /// Casts the `Offset` to a different lifetime.
    ///
    /// This is *safe* because an offset by itself has no guarantees associated with it.
    #[inline(always)]
    pub fn cast<'p2, 'v2>(&self) -> Offset<'p2, 'v2> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    /// Gets the offset as a `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hoard::offset::Offset;
    ///
    /// assert_eq!(Offset::new(0).unwrap().get(), 0);
    /// assert_eq!(Offset::new(1).unwrap().get(), 1);
    /// ```
    #[inline(always)]
    pub fn get(&self) -> usize {
        (self.raw.get().get() >> 1) as usize
    }

    /// Creates a dangling `Offset`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hoard::offset::Offset;
    ///
    /// assert_eq!(Offset::dangling().get(), Offset::MAX);
    /// ```
    #[inline(always)]
    pub fn dangling() -> Self {
        Self::new(Self::MAX).unwrap()
    }

    /// Erases the lifetime of an `Offset`.
    pub fn to_static(&self) -> Offset<'static, 'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }
}

/*
/// Enum for the kinds of `OffsetMut`.
#[derive(Debug)]
pub enum Kind<'p, 'v> {
    /// An unmodified `Offset`.
    Offset(Offset<'p, 'v>),

    /// A pointer to something in the heap.
    Ptr(HeapPtr),
}

impl<'p, 'v, A> OffsetMut<'p, 'v, A> {
    /// Create an `OffsetMut` from a pointer.
    ///
    /// Returns `None` if the alignment is incorrect.
    #[inline]
    pub fn from_ptr(ptr: NonNull<u16>) -> Option<Self> {
        let raw = ptr.as_ptr() as usize as u64;

        if raw & 1 == 1 {
            unsafe { Some(mem::transmute(ptr.as_ptr() as usize as u64)) }
        } else {
            None
        }
    }

    /// Creates an `OffsetMut` from a pointer without checking the alignment.
    ///
    /// # Safety
    ///
    /// The pointer must be properly aligned.
    #[inline]
    pub unsafe fn from_ptr_unchecked(ptr: NonNull<u16>) -> Self {
        match Self::from_ptr(ptr) {
            Some(this) => this,
            None => {
                unreachable_unchecked()
            }
        }
    }

    /// Returns the kind of offset.
    pub fn kind(&self) -> Kind<'p, 'v> {
        if self.inner.raw.get().get() & 1 == 1 {
            Kind::Offset(self.inner)
        } else {
            Kind::Ptr(unsafe { mem::transmute(self.inner) })
        }
    }

    /// Gets the `Offset` from a clean `OffsetMut`.
    #[inline(always)]
    pub fn get_offset(&self) -> Option<Offset<'p, 'v>> {
        match self.kind() {
            Kind::Offset(offset) => Some(offset),
            Kind::Ptr(_) => None,
        }
    }

    /// Gets the pointer from a dirty `OffsetMut`.
    #[inline(always)]
    pub fn get_ptr(&self) -> Option<HeapPtr> {
        match self.kind() {
            Kind::Ptr(ptr) => Some(ptr),
            Kind::Offset(_) => None,
        }
    }
}

/*
impl<'p, 'v, A> AsPtr<OffsetMut<'p, 'v, A>> for HeapPtr {
    #[inline(always)]
    fn as_ptr(&self) -> &OffsetMut<'p, 'v, A> {
        static_assertions::assert_eq_size!(OffsetMut, HeapPtr);
        unsafe {
            &*(self as *const _ as *const _)
        }
    }
}
*/

impl<'p, 'v> Ptr for OffsetMut<'p, 'v> {
    type Zone = TryPile<'p, 'v>;
    type BlobZone = TryPile<'p, 'v>;
    type Persist = Offset<'p, 'v>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        match self.kind() {
            Kind::Offset(_) => {},
            Kind::Ptr(heap_ptr) => heap_ptr.dealloc::<T>(metadata),
        }
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        match self.kind() {
            Kind::Ptr(ptr) => {
                todo!()
            },
            Kind::Offset(offset) => Err(offset),
        }
    }
}

impl<'p,'v> Default for OffsetMut<'p, 'v> {
    fn default() -> Self {
        Offset::dangling().into()
    }
}

#[derive(Debug, Default)]
pub struct ShallowDumper<'p, 'v> {
    marker: PhantomData<OffsetMut<'p, 'v>>,
    written: Vec<u8>,
    initial_offset: usize,
}

impl<'p, 'v> Saver for ShallowDumper<'p, 'v> {
    type SrcPtr = OffsetMut<'p, 'v>;
    type DstPtr = Offset<'p, 'v>;
    type Error = !;

    fn try_save_raw<R, T: ?Sized + ValidateBlob>(&self,
        ptr: &Offset<'p, 'v>,
        _metadata: T::Metadata,
        _f: impl FnOnce(ValidBlob<T>, &<Self::SrcPtr as Ptr>::BlobZone) -> R,
    ) -> Result<Result<<Self::DstPtr as Ptr>::Persist, R>,
                Self::Error>
    {
        Ok(Ok(*ptr))
    }


    fn finish_save<T>(&mut self, value_poll: &T) -> Result<Offset<'p, 'v>, Self::Error>
        where T: EncodeBlob
    {
        let offset = self.initial_offset
                         .checked_add(self.written.len())
                         .and_then(Offset::new)
                         .expect("overflow");

        let written = mem::replace(&mut self.written, vec![]);
        self.written = value_poll.encode_blob(written).into_ok();
        Ok(offset)
    }
}

impl<'p, 'v> ShallowDumper<'p, 'v> {
    pub fn new(initial_offset: usize) -> Self {
        Self {
            marker: PhantomData,
            written: vec![],
            initial_offset,
        }
    }

    pub fn from_buf(buf: impl Into<Vec<u8>>) -> Self {
        Self {
            marker: PhantomData,
            initial_offset: 0,
            written: buf.into(),
        }
    }

    pub fn save<T: ?Sized>(mut self, value: &T) -> (Vec<u8>, Offset<'p, 'v>)
        where T: SavePtr<OffsetMut<'p, 'v>, Offset<'p, 'v>>
    {
        let mut encoder = value.init_save_ptr();
        encoder.save_poll(&mut self).into_ok();
        let offset = self.finish_save(&encoder).into_ok();
        (self.written, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bag::Bag;

    #[test]
    fn test_shallow_dumper() {
        let (buf, offset) = ShallowDumper::new(0).save(&42u8);
        //assert_eq!(offset, 0);
        //assert_eq!(buf, &[42]);

        /*
        let own = OffsetMut::alloc(42u8);

        let (buf, offset) = ShallowDumper::new(0).save(&own);
        assert_eq!(offset, 1);
        assert_eq!(buf, &[42, 1,0,0,0,0,0,0,0]);

        let own2 = OffsetMut::alloc(own);
        let (buf, offset) = ShallowDumper::new(0).save(&own2);
        assert_eq!(offset, 9);
        assert_eq!(buf,
            &[42,
              1,0,0,0,0,0,0,0,
              3,0,0,0,0,0,0,0,
            ]);
        */
    }
}
*/
