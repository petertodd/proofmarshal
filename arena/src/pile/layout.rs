use core::ops::Range;

/// Layout of a fixed-size value in a pile.
#[derive(Default,Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct Layout {
    len: usize,
    niche_start: usize,
    niche_end: usize,
}

impl Layout {
    /// Creates a new `Layout` with a given length.
    pub const fn new(len: usize) -> Self {
        Self {
            len,
            niche_start: 0,
            niche_end: 0,
        }
    }

    /// Creates a non-zero layout.
    ///
    /// The entire length will be considered a non-zero niche.
    pub const fn new_nonzero(len: usize) -> Self {
        Self {
            len,
            niche_start: 0,
            niche_end: len,
        }
    }

    /// Creates a layout with a non-zero niche.
    pub const fn with_niche(len: usize, niche: Range<usize>) -> Self {
        // HACK: since we don't have const panic yet...
        let _ = niche.end - niche.start - 1;
        let _: usize = (niche.end > niche.start) as usize - 1;
        Self {
            len,
            niche_start: niche.start,
            niche_end: niche.end,
        }
    }

    /// Gets the length in bytes.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Creates a layout describing `self` followed by `next`.
    ///
    /// If either `self` or `next` have a non-zero niche, the niche with the shortest length will
    /// be used; if the lengths are the same the first niche is used.
    pub const fn extend(self, next: Layout) -> Self {
        let len = self.len + next.len;

        let niche_starts = [self.niche_start, self.len + next.niche_start];
        let niche_ends = [self.niche_end, self.len + next.niche_end];

        let niche_len1 = self.niche_end - self.niche_start;
        let niche_len2 = next.niche_end - next.niche_start;

        let i = ((niche_len2 != 0) & (niche_len2 < niche_len1)) as usize;

        Self {
            len,
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
        let l = Layout::new(0);
        assert_eq!(l.len, 0);
        assert_eq!(l.len(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = Layout::new_nonzero(0);
        assert_eq!(l.len, 0);
        assert_eq!(l.len(), 0);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 0);
        assert_eq!(l.niche(), None);

        let l = Layout::new_nonzero(42);
        assert_eq!(l.len, 42);
        assert_eq!(l.len(), 42);
        assert_eq!(l.niche_start, 0);
        assert_eq!(l.niche_end, 42);
        assert_eq!(l.niche(), Some(0..42));
    }

    #[test]
    fn extend() {
        assert_eq!(Layout::new(0).extend(Layout::new(0)),
                   Layout::new(0));

        assert_eq!(Layout::new(1).extend(Layout::new(3)),
                   Layout::new(4));

        // smallest niche picked
        assert_eq!(Layout::new_nonzero(1).extend(Layout::new_nonzero(3)),
                   Layout { len: 4, niche_start: 0, niche_end: 1 });

        // smallest niche picked
        assert_eq!(Layout::new_nonzero(3).extend(Layout::new_nonzero(1)),
                   Layout { len: 4, niche_start: 3, niche_end: 4 });

        // equal size niches, so first niche picked
        assert_eq!(Layout::new_nonzero(3).extend(Layout::new_nonzero(3)),
                   Layout { len: 6, niche_start: 0, niche_end: 3 });
    }
}
