use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct PaddingError(());

pub struct Blob<'a, T: ?Sized + Pointee, P> {
    marker: PhantomData<(fn() -> &'a T, fn() -> P)>,
    ptr: *const u8,
    metadata: T::Metadata,
}

pub struct ValidBlob<'a, T: ?Sized + Pointee, P>(Blob<'a, T, P>);

pub struct FullyValidBlob<'a, T: ?Sized + Pointee, P>(ValidBlob<'a, T, P>);

impl<'a, T: ?Sized + Pointee, P> Blob<'a, T, P> {
    pub fn new(buf: &'a [u8], metadata: T::Metadata) -> Option<Self>
        where T: Save<P>
    {
        if buf.len() == T::blob_layout(metadata).size() {
            unsafe { Some(Self::new_unchecked(buf, metadata)) }
        } else {
            None
        }
    }

    pub unsafe fn new_unchecked(buf: &'a [u8], metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr: buf.as_ptr(),
            metadata,
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<'a, T: ?Sized + Load<P>, P> Blob<'a, T, P> {
    pub fn validate(self) -> impl ValidateFields<'a, T, P> {
        FieldCursor {
            blob: self,
            offset: 0,
        }
    }

    pub fn validate_enum(self) -> (u8, impl ValidateVariant<'a, T, P>) {
        (self[0],
         FieldCursor {
             blob: self,
             offset: 1,
         })
    }

    pub fn assume_valid(self, state: T::ValidateChildren) -> BlobValidator<'a, T, P> {
        BlobValidator {
            blob: ValidBlob(self),
            state,
        }
    }

    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a, T, P> {
        ValidBlob(self).assume_fully_valid()
    }
}

impl<'a, T: ?Sized + Save<P>, P> Blob<'a, T, P> {
    fn as_bytes(&self) -> &'a [u8] {
        unsafe {
            slice::from_raw_parts(self.ptr,
                                  T::blob_layout(self.metadata).size())
        }
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for Blob<'a, T, P> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            ptr: self.ptr,
            metadata: self.metadata,
        }
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for Blob<'a, T, P> {}

/*
#[derive(Debug)]
pub struct TryFromBlobError(());

impl<'a, T: Load<Z>, Z: Zone> TryFrom<&'a [u8]> for Blob<'a, T, Z> {
    type Error = TryFromBlobError;
    fn try_from(buf: &'a [u8]) -> Result<Self, TryFromBlobError> {
        Self::new(buf, T::make_sized_metadata())
             .ok_or(TryFromBlobError(()))
    }
}
*/

impl<T: ?Sized + Save<P>, P> ops::Deref for Blob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: ?Sized + Save<P>, P> ops::Deref for ValidBlob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl<T: ?Sized + Save<P>, P> ops::Deref for FullyValidBlob<'_, T, P> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

/*
impl<T: ?Sized + Pointee, Z> fmt::Debug for Blob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.ptr)
            .field("metadata", &self.metadata)
            .finish()
    }
}
*/

pub trait ValidateFields<'a, T: ?Sized + Load<P>, P> {
    fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error>;
    fn done(self, state: T::ValidateChildren) -> BlobValidator<'a, T, P>;
}

pub trait ValidateVariant<'a, T: ?Sized + Load<P>, P> {
    fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error>;
    fn done(self, state: T::ValidateChildren)
        -> Result<BlobValidator<'a, T, P>, PaddingError>;
}

pub trait DecodeFields<'a, T: ?Sized + Load<P>, P> {
    fn field<F: 'a + Decode<P>>(&mut self) -> F;
}

struct FieldCursor<'a, T: ?Sized + Pointee, P> {
    blob: Blob<'a, T, P>,
    offset: usize,
}

impl<T: ?Sized + Load<P>, P> fmt::Debug for FieldCursor<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("buf", &&self.blob[..])
            .field("metadata", &self.blob.metadata)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a, T: ?Sized + Load<P>, P> FieldCursor<'a, T, P> {
    fn field_blob<F: Decode<P>>(&mut self) -> Blob<'a, F, P> {
        let size = F::BLOB_LAYOUT.size();

        let start = self.offset;
        self.offset += size;

        let buf = self.blob.as_bytes().get(start .. self.offset)
                                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }

    fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }
}

impl<'a, T: ?Sized + Load<P>, P> ValidateFields<'a, T, P> for FieldCursor<'a, T, P> {
    fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.field::<F>()
    }

    fn done(self, state: T::ValidateChildren) -> BlobValidator<'a, T, P> {
        assert_eq!(self.offset, self.blob.len(),
                   "not fully validated");

        self.blob.assume_valid(state)
    }
}
impl<'a, T: ?Sized + Load<P>, P> ValidateVariant<'a, T, P> for FieldCursor<'a, T, P> {
    fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.field::<F>()
    }

    fn done(self, state: T::ValidateChildren) -> Result<BlobValidator<'a, T, P>, PaddingError> {
        if self.blob[self.offset .. ].iter().all(|b| *b == 0) {
            Ok(self.blob.assume_valid(state))
        } else {
            Err(PaddingError(()))
        }
    }
}

impl<'a, T: ?Sized + Pointee, P> ValidBlob<'a,T,P> {
    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a,T,P> {
        FullyValidBlob(self)
    }

    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for ValidBlob<'a, T, P> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for ValidBlob<'a, T, P> {}

impl<T: ?Sized + Pointee, P> fmt::Debug for ValidBlob<'_, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.0.ptr)
            .field("metadata", &self.0.metadata)
            .finish()
    }
}

pub struct BlobValidator<'a, T: ?Sized + Load<P>, P> {
    blob: ValidBlob<'a, T, P>,
    state: T::ValidateChildren,
}

/*
impl<'a, T: ?Sized + Load<Z>, Z: Zone> BlobValidator<'a, T, Z> {
    pub fn poll<V>(&mut self, ptr_validator: &mut V) -> Poll<Result<FullyValidBlob<'a, T, Z>, V::Error>>
        where V: ValidatePtr<Z>
    {
        self.state.validate_children(ptr_validator)
            .map_ok(|()|
                unsafe { self.blob.assume_fully_valid() }
            )
    }
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> fmt::Debug for BlobValidator<'a, T, Z>
where T::ValidateChildren: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BlobValidator")
            .field("blob", &self.blob)
            .field("state", &self.state)
            .finish()
    }
}


impl<T: ?Sized + Pointee, Z> fmt::Debug for FullyValidBlob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &(self.0).0.ptr)
            .field("metadata", &(self.0).0.metadata)
            .finish()
    }
}
*/

impl<'a, T: ?Sized + Pointee, P> FullyValidBlob<'a,T,P> {
    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata()
    }
}

impl<'a, T: ?Sized + Pointee + Owned, P> FullyValidBlob<'a,T,P> {
    pub unsafe fn assume_valid(self) -> &'a T {
        &*T::make_fat_ptr((self.0).0.ptr as *const (), self.metadata())
    }

    pub unsafe fn assume_valid_ref(self) -> Ref<'a, T> {
        Ref::Borrowed(self.assume_valid())
    }
}

impl<'a, T: ?Sized + Pointee, P> Clone for FullyValidBlob<'a, T, P> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, P> Copy for FullyValidBlob<'a, T, P> {}

impl<'a, T: ?Sized + Load<P>, P> FullyValidBlob<'a, T, P> {
    pub fn decode_struct(self, loader: impl Loader<P>) -> impl DecodeFields<'a,T,P> {
        FieldDecoder {
             cursor: FieldCursor {
                 blob: (self.0).0,
                 offset: 0,
             },
            loader,
        }
    }

    pub fn decode_enum(self, loader: impl Loader<P>) -> (u8, impl DecodeFields<'a,T,P>) {
        (self[0],
         FieldDecoder {
             cursor: FieldCursor {
                 blob: (self.0).0,
                 offset: 1,
             },
            loader,
         })
    }
}

struct FieldDecoder<'a, T: ?Sized + Pointee, P, L> {
    cursor: FieldCursor<'a, T, P>,
    loader: L,
}

impl<'a, T: ?Sized + Load<P>, P, L> DecodeFields<'a, T, P> for FieldDecoder<'a, T, P, L>
where L: Loader<P>,
{
    fn field<F: 'a + Decode<P>>(&mut self) -> F {
        let blob = self.cursor.field_blob::<F>();
        let blob = unsafe { blob.assume_fully_valid() };

        F::decode_blob(blob, &self.loader)
          .take_sized()
    }
}


pub trait WriteBlob : Sized {
    type Ok;
    type Error;

    /// Write an encodable value.
    #[inline(always)]
    fn write_primitive<T: Primitive>(self, value: &T) -> Result<Self, Self::Error> {
        self.write::<!, T>(value)
    }

    /// Write an encodable value.
    #[inline(always)]
    fn write<P, E: EncodePoll<P>>(self, encoder: &E) -> Result<Self, Self::Error> {
        let size = E::TARGET_BLOB_LAYOUT.size();
        let value_writer = ValueWriter::new(self, size);
        encoder.encode_blob(value_writer)
    }

    /// Writes bytes to the blob.
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;

    /// Writes padding bytes to the blob.
    #[inline(always)]
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for _ in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }

    /// Finishes writing the blob.
    ///
    /// Will panic if the correct number of bytes hasn't been written.
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

struct ValueWriter<W> {
    inner: W,
    remaining: usize,
}

impl<W> ValueWriter<W> {
    #[inline(always)]
    fn new(inner: W, size: usize) -> Self {
        Self {
            inner,
            remaining: size,
        }
    }
}

impl<W: WriteBlob> WriteBlob for ValueWriter<W> {
    type Ok = W;
    type Error = W::Error;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        let remaining = self.remaining.checked_sub(src.len())
                                      .expect("overflow");
        Ok(Self::new(self.inner.write_bytes(src)?,
                     remaining))
    }

    #[inline(always)]
    fn write_padding(self, len: usize) -> Result<Self, Self::Error> {
        let remaining = self.remaining.checked_sub(len)
                                      .expect("overflow");
        Ok(Self::new(self.inner.write_padding(len)?,
                     remaining))
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.remaining, 0,
                   "not all bytes written");
        Ok(self.inner)
    }
}

impl WriteBlob for &'_ mut [u8] {
    type Ok = ();
    type Error = !;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        if self.len() < src.len() {
            panic!("overflow")
        };

        let (dst, rest) = self.split_at_mut(src.len());
        dst.copy_from_slice(src);
        Ok(rest)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.len(), 0,
                   "not all bytes written");
        Ok(())
    }
}

impl WriteBlob for &'_ mut [MaybeUninit<u8>] {
    type Ok = ();
    type Error = !;

    #[inline(always)]
    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error> {
        if self.len() < src.len() {
            panic!("overflow")
        };

        let (dst, rest) = self.split_at_mut(src.len());

        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_ptr() as *mut u8, src.len());
        }

        Ok(rest)
    }

    #[inline(always)]
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        assert_eq!(self.len(), 0,
                   "not all bytes written");
        Ok(())
    }
}

/// Encoding of a fixed-size value in a pile.
#[derive(Default,Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct BlobLayout {
    size: usize,
    niche_start: usize,
    niche_end: usize,
    pub(crate) inhabited: bool,
}

impl BlobLayout {
    /// Creates a new `Encoding` with a given length.
    pub const fn new(size: usize) -> Self {
        Self {
            size,
            niche_start: 0,
            niche_end: 0,
            inhabited: true,
        }
    }

    /// Creates a non-zero layout.
    ///
    /// The entire length will be considered a non-zero niche.
    pub const fn new_nonzero(size: usize) -> Self {
        Self {
            size,
            niche_start: 0,
            niche_end: size,
            inhabited: true,
        }
    }

    pub(crate) const fn never() -> Self {
        Self {
            size: 0,
            niche_start: 0,
            niche_end: 0,
            inhabited: false,
        }
    }

    /// Creates a layout with a non-zero niche.
    pub const fn with_niche(size: usize, niche: Range<usize>) -> Self {
        // HACK: since we don't have const panic yet...
        let _ = niche.end - niche.start - 1;
        let _: usize = (niche.end > niche.start) as usize - 1;
        Self {
            size,
            niche_start: niche.start,
            niche_end: niche.end,
            inhabited: true,
        }
    }

    /// Gets the size in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    pub const fn inhabited(self) -> bool {
        self.inhabited
    }

    /// Creates a layout describing `self` followed by `next`.
    ///
    /// If either `self` or `next` have a non-zero niche, the niche with the shortest length will
    /// be used; if the lengths are the same the first niche is used.
    pub const fn extend(self, next: BlobLayout) -> Self {
        let size = self.size + next.size;

        let niche_starts = [self.niche_start, self.size + next.niche_start];
        let niche_ends = [self.niche_end, self.size + next.niche_end];

        let niche_size1 = self.niche_end - self.niche_start;
        let niche_size2 = next.niche_end - next.niche_start;

        let i = ((niche_size2 != 0) & (niche_size2 < niche_size1)) as usize;

        Self {
            size,
            niche_start: niche_starts[i],
            niche_end: niche_ends[i],
            inhabited: self.inhabited & next.inhabited,
        }
    }

    pub const fn has_niche(self) -> bool {
        self.inhabited & (self.niche_start != self.niche_end)
    }

    /// Gets the non-zero niche, if present.
    pub fn niche(self) -> Option<Range<usize>> {
        if self.has_niche() {
            Some(self.niche_start .. self.niche_end)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_exact_u8_slice() -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0,0,0];

        let w = &mut buf[..];
        w.write_bytes(&[1])?
         .write_bytes(&[2])?
         .write_bytes(&[3])?
         .finish()?;

        assert_eq!(buf, [1,2,3]);

        Ok(())
    }

    #[test]
    fn layout_new() {
        let l = BlobLayout::new(0);
        assert_eq!(l.size, 0);
        assert_eq!(l.size(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = BlobLayout::new_nonzero(0);
        assert_eq!(l.size, 0);
        assert_eq!(l.size(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = BlobLayout::new_nonzero(42);
        assert_eq!(l.size, 42);
        assert_eq!(l.size(), 42);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 42);
        assert_eq!(l.niche(), Some(0..42));
    }

    #[test]
    fn extend() {
        assert_eq!(BlobLayout::new(0).extend(BlobLayout::new(0)),
                   BlobLayout::new(0));

        assert_eq!(BlobLayout::new(1).extend(BlobLayout::new(3)),
                   BlobLayout::new(4));

        // smallest niche picked
        assert_eq!(BlobLayout::new_nonzero(1).extend(BlobLayout::new_nonzero(3)),
                   BlobLayout { size: 4, niche_start: 0, niche_end: 1, inhabited: true, });

        // smallest niche picked
        assert_eq!(BlobLayout::new_nonzero(3).extend(BlobLayout::new_nonzero(1)),
                   BlobLayout { size: 4, niche_start: 3, niche_end: 4, inhabited: true, });

        // equal size niches, so first niche picked
        assert_eq!(BlobLayout::new_nonzero(3).extend(BlobLayout::new_nonzero(3)),
                   BlobLayout { size: 6, niche_start: 0, niche_end: 3, inhabited: true, });
    }
}
