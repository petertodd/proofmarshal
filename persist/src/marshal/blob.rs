use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::{self, Range};
use core::ptr::{self, NonNull};
use core::slice;

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct PaddingError(());

pub struct Blob<'a, T: ?Sized + Pointee, Z> {
    marker: PhantomData<(fn() -> &'a T, fn() -> Z)>,
    ptr: *const u8,
    metadata: T::Metadata,
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> Blob<'a, T, Z> {
    pub fn new(buf: &'a [u8], metadata: T::Metadata) -> Option<Self> {
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

    pub fn validate(self) -> impl ValidateFields<'a, T, Z> {
        FieldCursor {
            blob: self,
            offset: 0,
        }
    }

    pub fn validate_enum(self) -> (u8, impl ValidateVariant<'a, T, Z>) {
        (self[0],
         FieldCursor {
             blob: self,
             offset: 1,
         })
    }

    pub fn assume_valid(self, state: T::ValidateChildren) -> ValidateBlob<'a, T, Z> {
        ValidateBlob {
            blob: ValidBlob(self),
            state,
        }
    }

    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a, T, Z> {
        ValidBlob(self).assume_fully_valid()
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        unsafe {
            slice::from_raw_parts(self.ptr,
                                  T::blob_layout(self.metadata).size())
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<'a, T: ?Sized + Pointee, Z> Clone for Blob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            ptr: self.ptr,
            metadata: self.metadata,
        }
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for Blob<'a, T, Z> {}

#[derive(Debug)]
pub struct TryFromBlobError(());

impl<'a, T: Load<Z>, Z: Zone> TryFrom<&'a [u8]> for Blob<'a, T, Z> {
    type Error = TryFromBlobError;
    fn try_from(buf: &'a [u8]) -> Result<Self, TryFromBlobError> {
        Self::new(buf, T::make_sized_metadata())
             .ok_or(TryFromBlobError(()))
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> ops::Deref for Blob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: ?Sized + Pointee, Z> fmt::Debug for Blob<'_, T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("ptr", &self.ptr)
            .field("metadata", &self.metadata)
            .finish()
    }
}

pub trait ValidateFields<'a, T: ?Sized + Load<Z>, Z: Zone> {
    fn field<F: 'a + Load<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error>;
    fn done(self, state: T::ValidateChildren) -> ValidateBlob<'a, T, Z>;
}

pub trait ValidateVariant<'a, T: ?Sized + Load<Z>, Z: Zone> {
    fn field<F: 'a + Load<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error>;
    fn done(self, state: T::ValidateChildren)
        -> Result<ValidateBlob<'a, T, Z>, PaddingError>;
}

pub trait DecodeFields<'a, T: ?Sized + Load<Z>, Z: Zone> {
    fn field<F: 'a + Load<Z>>(&mut self) -> F;
}

struct FieldCursor<'a, T: ?Sized + Pointee, Z> {
    blob: Blob<'a, T, Z>,
    offset: usize,
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> FieldCursor<'a, T, Z> {
    fn field_blob<F: Load<Z>>(&mut self) -> Blob<'a, F, Z> {
        let size = F::BLOB_LAYOUT.size();

        let start = self.offset;
        self.offset += size;

        let buf = self.blob.as_bytes().get(start .. self.offset)
                                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> ValidateFields<'a, T, Z> for FieldCursor<'a, T, Z> {
    fn field<F: 'a + Load<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }

    fn done(self, state: T::ValidateChildren) -> ValidateBlob<'a, T, Z> {
        assert_eq!(self.offset, self.blob.len(),
                   "not fully validated");

        self.blob.assume_valid(state)
    }
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> ValidateVariant<'a, T, Z> for FieldCursor<'a, T, Z> {
    fn field<F: 'a + Load<Z>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }

    fn done(self, state: T::ValidateChildren) -> Result<ValidateBlob<'a, T, Z>, PaddingError> {
        if self.blob[self.offset .. ].iter().all(|b| *b == 0) {
            Ok(self.blob.assume_valid(state))
        } else {
            Err(PaddingError(()))
        }
    }
}



#[derive(Debug)]
pub struct ValidBlob<'a, T: ?Sized + Pointee, Z>(Blob<'a, T, Z>);

impl<'a, T: ?Sized + Pointee, Z> ValidBlob<'a,T,Z> {
    pub unsafe fn assume_fully_valid(self) -> FullyValidBlob<'a,T,Z> {
        FullyValidBlob(self)
    }

    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata
    }
}

impl<'a, T: ?Sized + Pointee, Z> Clone for ValidBlob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for ValidBlob<'a, T, Z> {}

impl<T: ?Sized + Load<Z>, Z: Zone> ops::Deref for ValidBlob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

pub struct ValidateBlob<'a, T: ?Sized + Load<Z>, Z: Zone> {
    blob: ValidBlob<'a, T, Z>,
    state: T::ValidateChildren,
}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> ValidateBlob<'a, T, Z> {
    pub fn poll<V>(&mut self, ptr_validator: V) -> Poll<Result<FullyValidBlob<'a, T, Z>, V::Error>>
        where V: ValidatePtr<Z>
    {
        self.state.validate_children(ptr_validator)
            .map_ok(|()|
                unsafe { self.blob.assume_fully_valid() }
            )
    }
}

#[derive(Debug)]
pub struct FullyValidBlob<'a, T: ?Sized + Pointee, Z>(ValidBlob<'a, T, Z>);

impl<'a, T: ?Sized + Pointee, Z> FullyValidBlob<'a,T,Z> {
    pub fn metadata(&self) -> T::Metadata {
        self.0.metadata()
    }
}

impl<'a, T: ?Sized + Pointee + Owned, Z> FullyValidBlob<'a,T,Z> {
    pub unsafe fn assume_valid(self) -> &'a T {
        &*T::make_fat_ptr((self.0).0.ptr as *const (), self.metadata())
    }

    pub unsafe fn assume_valid_ref(self) -> Ref<'a, T> {
        Ref::Borrowed(self.assume_valid())
    }
}

impl<'a, T: ?Sized + Pointee, Z> Clone for FullyValidBlob<'a, T, Z> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T: ?Sized + Pointee, Z> Copy for FullyValidBlob<'a, T, Z> {}

impl<'a, T: ?Sized + Load<Z>, Z: Zone> FullyValidBlob<'a, T, Z> {
    pub fn decode_struct(self, loader: impl Loader<Z>) -> impl DecodeFields<'a,T,Z> {
        FieldDecoder {
             cursor: FieldCursor {
                 blob: (self.0).0,
                 offset: 1,
             },
            loader,
        }
    }

    pub fn decode_enum(self, loader: impl Loader<Z>) -> (u8, impl DecodeFields<'a,T,Z>) {
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

struct FieldDecoder<'a, T: ?Sized + Pointee, Z, L> {
    cursor: FieldCursor<'a, T, Z>,
    loader: L,
}

impl<'a, T: ?Sized + Load<Z>, Z, L> DecodeFields<'a, T, Z> for FieldDecoder<'a, T, Z, L>
where Z: Zone,
      L: Loader<Z>,
{
    fn field<F: 'a + Load<Z>>(&mut self) -> F {
        let blob = self.cursor.field_blob::<F>();
        let blob = unsafe { blob.assume_fully_valid() };

        F::decode_blob(blob, &self.loader)
          .take_sized()
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> ops::Deref for FullyValidBlob<'_, T, Z> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        (self.0).0.as_bytes()
    }
}

pub trait WriteBlob : Sized {
    type Done;
    type Error;

    /// Write an encodable value.
    #[inline(always)]
    fn write<E: SavePoll>(self, encoder: &E) -> Result<Self, Self::Error> {
        let size = E::Target::BLOB_LAYOUT.size();
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
    fn done(self) -> Result<Self::Done, Self::Error>;
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
    type Done = W;
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
    fn done(self) -> Result<Self::Done, Self::Error> {
        assert_eq!(self.remaining, 0,
                   "not all bytes written");
        Ok(self.inner)
    }
}

impl WriteBlob for &'_ mut [u8] {
    type Done = ();
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
    fn done(self) -> Result<Self::Done, Self::Error> {
        assert_eq!(self.len(), 0,
                   "not all bytes written");
        Ok(())
    }
}

impl WriteBlob for &'_ mut [MaybeUninit<u8>] {
    type Done = ();
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
    fn done(self) -> Result<Self::Done, Self::Error> {
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
    inhabited: bool,
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
         .done()?;

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
