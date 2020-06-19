use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use super::*;

/// Encoding of a fixed-size value in a pile.
#[derive(Default,Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct BlobLayout {
    pub(crate) size: usize,
    pub(crate) niche_start: usize,
    pub(crate) niche_end: usize,
    pub(crate) inhabited: bool,
}

impl BlobLayout {
    /// Creates a new `Encoding` with a given length.
    #[inline(always)]
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
    #[inline(always)]
    pub const fn new_nonzero(size: usize) -> Self {
        Self {
            size,
            niche_start: 0,
            niche_end: size,
            inhabited: true,
        }
    }

    #[inline(always)]
    pub(crate) const fn never() -> Self {
        Self {
            size: 0,
            niche_start: 0,
            niche_end: 0,
            inhabited: false,
        }
    }

    /// Creates a layout with a non-zero niche.
    #[inline(always)]
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
    #[inline(always)]
    pub const fn size(self) -> usize {
        self.size
    }

    #[inline(always)]
    pub const fn inhabited(self) -> bool {
        self.inhabited
    }

    /// Creates a layout describing `self` followed by `next`.
    ///
    /// If either `self` or `next` have a non-zero niche, the niche with the shortest length will
    /// be used; if the lengths are the same the first niche is used.
    #[inline(always)]
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

    #[inline(always)]
    pub const fn has_niche(self) -> bool {
        self.inhabited & (self.niche_start != self.niche_end)
    }

    /// Gets the non-zero niche, if present.
    #[inline(always)]
    pub fn niche(self) -> Option<Range<usize>> {
        if self.has_niche() {
            Some(self.niche_start .. self.niche_end)
        } else {
            None
        }
    }
}

/*
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_exact_u8_slice() -> Result<(), !> {
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
*/
