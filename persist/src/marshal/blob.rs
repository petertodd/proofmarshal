use core::mem::{self, MaybeUninit};
use core::ops::Range;
use core::ptr;

use super::*;

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
