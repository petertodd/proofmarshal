use core::ops::Range;

/// Encoding of a fixed-size value in a pile.
#[derive(Default,Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct PileEncoding {
    size: usize,
    niche_start: usize,
    niche_end: usize,
}

impl PileEncoding {
    /// Creates a new `Encoding` with a given length.
    pub const fn new(size: usize) -> Self {
        Self {
            size,
            niche_start: 0,
            niche_end: 0,
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
        }
    }

    /// Gets the size in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Creates a layout describing `self` followed by `next`.
    ///
    /// If either `self` or `next` have a non-zero niche, the niche with the shortest length will
    /// be used; if the lengths are the same the first niche is used.
    pub const fn extend(self, next: PileEncoding) -> Self {
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
        }
    }

    /// Gets the non-zero niche, if present.
    pub fn niche(self) -> Option<Range<usize>> {
        if self.niche_start != self.niche_end {
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
    fn layout_new() {
        let l = PileEncoding::new(0);
        assert_eq!(l.size, 0);
        assert_eq!(l.size(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = PileEncoding::new_nonzero(0);
        assert_eq!(l.size, 0);
        assert_eq!(l.size(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = PileEncoding::new_nonzero(42);
        assert_eq!(l.size, 42);
        assert_eq!(l.size(), 42);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 42);
        assert_eq!(l.niche(), Some(0..42));
    }

    #[test]
    fn extend() {
        assert_eq!(PileEncoding::new(0).extend(PileEncoding::new(0)),
                   PileEncoding::new(0));

        assert_eq!(PileEncoding::new(1).extend(PileEncoding::new(3)),
                   PileEncoding::new(4));

        // smallest niche picked
        assert_eq!(PileEncoding::new_nonzero(1).extend(PileEncoding::new_nonzero(3)),
                   PileEncoding { size: 4, niche_start: 0, niche_end: 1 });

        // smallest niche picked
        assert_eq!(PileEncoding::new_nonzero(3).extend(PileEncoding::new_nonzero(1)),
                   PileEncoding { size: 4, niche_start: 3, niche_end: 4 });

        // equal size niches, so first niche picked
        assert_eq!(PileEncoding::new_nonzero(3).extend(PileEncoding::new_nonzero(3)),
                   PileEncoding { size: 6, niche_start: 0, niche_end: 3 });
    }
}
