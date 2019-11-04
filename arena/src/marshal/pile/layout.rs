use core::ops::Range;

#[derive(Default,Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct Layout {
    len: usize,
    niche_start: usize,
    niche_end: usize,
}

impl Layout {
    #[inline(always)]
    pub fn new(len: usize) -> Self {
        Self::with_niche(len, 0 .. 0)
    }

    #[inline(always)]
    pub fn new_nonzero(len: usize) -> Self {
        Self::with_niche(len, 0 .. len)
    }

    #[inline(always)]
    pub fn with_niche(len: usize, mut niche: Range<usize>) -> Self {
        assert!(niche.start <= niche.end);
        assert!(niche.end <= len);

        // Normalize niche
        if niche.start == niche.end {
            niche = 0 .. 0;
        }

        Self {
            len,
            niche_start: niche.start,
            niche_end: niche.end,
        }
    }

    #[inline(always)]
    pub fn niche(self) -> Option<Range<usize>> {
        if self.niche_start != self.niche_end {
            Some(self.niche_start .. self.niche_end)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn len(self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn extend(self, next: Layout) -> Self {
        let len = self.len().checked_add(next.len())
                            .expect("length overflow");

        match (self.niche(), next.niche()) {
            (None, None) => Self::new(len),
            (Some(niche), None) => Self::with_niche(len, niche),
            (None, Some(niche)) => {
                // Offset niche by our length
                Self::with_niche(len, niche.start + self.len() .. niche.end + self.len())
            }
            (Some(niche1), Some(niche2)) => {
                // Use the shortest niche.
                //
                // In the event of a tie, use the first niche.
                if niche1.end - niche1.start <= niche2.end - niche1.start {
                    Layout::with_niche(len, niche1)
                } else {
                    Layout::with_niche(len,
                                       niche2.start + self.len() .. niche2.end + self.len())
                }
            }
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

        let l = Layout::with_niche(11, 5 .. 6);
        assert_eq!(l.len(), 11);
        assert_eq!(l.niche(), Some(5..6));
    }

    #[test]
    #[should_panic]
    fn invalid_niche_end_out_of_bounds() {
        Layout::with_niche(1, 0 .. 2);
    }

    #[test]
    #[should_panic]
    fn invalid_niche_start_out_of_bounds() {
        Layout::with_niche(1, 2 .. 3);
    }

    #[test]
    #[should_panic]
    fn invalid_niche_start_after_end() {
        Layout::with_niche(10, 3 .. 2);
    }

    #[test]
    fn extend() {
        assert_eq!(Layout::new(0).extend(Layout::new(0)),
                   Layout::new(0));

        assert_eq!(Layout::new(1).extend(Layout::new(3)),
                   Layout::new(4));

        // smallest niche picked
        assert_eq!(Layout::new_nonzero(1).extend(Layout::new_nonzero(3)),
                   Layout::with_niche(4, 0 .. 1));

        // smallest niche picked
        assert_eq!(Layout::new_nonzero(3).extend(Layout::new_nonzero(1)),
                   Layout::with_niche(4, 3 .. 4));

        // equal size niches, so first niche picked
        assert_eq!(Layout::new_nonzero(3).extend(Layout::new_nonzero(3)),
                   Layout::with_niche(6, 0 .. 3));
    }
}
