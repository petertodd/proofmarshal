use std::cell::Cell;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::error;
use std::borrow::{Borrow, BorrowMut};
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::DerefMut;
use std::ptr;

use thiserror::Error;

use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::bag::Bag;
use hoard::primitive::Primitive;
use hoard::owned::{IntoOwned, Take, Ref, Own};
use hoard::pointee::Pointee;
use hoard::zone::{Alloc, Get, GetMut, Ptr, PtrBlob, Zone, AsZone};
use hoard::load::{Load, LoadRef, MaybeValid};

use crate::collections::leaf::Leaf;
use crate::collections::length::*;
use crate::collections::height::Height;
use crate::collections::perfecttree::PerfectTree;

pub mod peaktree;
use self::peaktree::PeakTree;

#[derive(Debug)]
pub struct MMR<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    peaks: Option<PeakTree<T, Z, P>>,
    zone: Z,
}

impl<T, Z: Zone> Default for MMR<T, Z>
where Z: Default
{
    fn default() -> Self {
        Self::new_in(Z::default())
    }
}

impl<T, Z: Zone> MMR<T, Z> {
    pub fn new_in(zone: Z) -> Self {
        Self {
            peaks: None,
            zone,
        }
    }

    pub fn len(&self) -> Length {
        self.peaks.as_ref()
            .map(|peaks| {
                peaks.len().into()
            }).unwrap_or(Length(0))
    }
}

impl<T, Z: Zone> MMR<T, Z>
where T: Load,
{
    pub fn try_push(&mut self, value: T) -> Result<(), T>
        where Z: GetMut + Alloc
    {
        if self.len() < Length::MAX {
            let leaf = Leaf::new_in(value, self.zone);
            match self.try_push_leaf(leaf) {
                Ok(()) => Ok(()),
                Err(_overflow) => unreachable!("overflow condition already checked"),
            }
        } else {
            Err(value)
        }
    }

    pub fn try_push_leaf(&mut self, leaf: Leaf<T, Z>) -> Result<(), Leaf<T, Z>>
        where Z: GetMut + Alloc
    {
        if self.len() < Length::MAX {
            let new_peak = if let Some(peaks) = self.peaks.take() {
                peaks.try_push_peak(leaf.into()).ok().expect("overflow condition already checked")
            } else {
                PeakTree::from(PerfectTree::from(leaf))
            };
            self.peaks = Some(new_peak);
            Ok(())
        } else {
            Err(leaf)
        }
    }

    pub fn get(&self, idx: usize) -> Option<Ref<T>>
        where Z: Get + AsZone<T::Zone>
    {
        self.get_leaf(idx).map(|leaf| {
            match leaf {
                Ref::Borrowed(leaf) => leaf.get(),
                Ref::Owned(_leaf) => todo!(),
            }
        })
    }

    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, Z>>>
        where Z: Get
    {
        match &self.peaks {
            Some(peaks) => {
                if let Some((height, idx_in_peak)) = idx_to_containing_height(peaks.len(), idx) {
                    peaks.get(height).and_then(|peak| {
                        match peak {
                            Ref::Borrowed(peak) => peak.get_leaf(idx_in_peak),
                            Ref::Owned(_peak) => todo!(),
                        }
                    })
                } else {
                    None
                }
            },
            None => None,
        }
    }
}

/// Determines the height of the peak containing a given index, as well as the index within that
/// peak tree.
///
/// Returns `None` if the index is out of bounds.
///
/// This is a separate function so it can be easily tested at the extremes.
fn idx_to_containing_height(len: NonZeroLength, idx: usize) -> Option<(Height, usize)> {
    let len = usize::from(len);
    if idx < len {
        // How does this work?
        //
        // The peaks tree is comprised of one or more perfect merkle trees. Thus each true bit i in
        // len represents a tree with 2^i items. Trees are merged together as new items are added
        // to the MMR, so we want to "skip past" all the trees prior to idx.
        //
        // Each true bit i in idx represents a tree with 2^i items *prior* to idx.
        //
        // That means that XOR *cancels out* the trees that exist in both len, and idx.
        //
        // For example, idx = 0 has no bits set, so there are no *prior* trees. 0 ^ len = len,
        // because no prior trees were cancelled out.
        let diff = len ^ idx;

        // With the common trees removed, leading_zeros() gives us the biggest remaining tree,
        // which will be the tree containing the idx that we want.
        //
        // For example, if the biggest tree is of height 2^i, indexes falling into that tree range
        // from 0 to 2^i - 1:
        //
        // len = 4 = 0b100
        // idx = 0 = 0b000
        // idx = 1 = 0b001
        // idx = 2 = 0b010
        // idx = 3 = 0b011
        // idx^len = 0b1xx
        //
        // Now suppose we have more than one tree:
        //
        // len = 15 = 0b1111 <- four trees, 2^3 + 2^2 + 2^1 + 2^0
        //
        // An item at idx = 14 is in the very last tree, 2^0, and is past the 2^3, 2^2, and 2^1
        // trees. XOR cancels out those trees, leaving just the last bit set, which corresponds to
        // the 2^0 tree:
        //
        // idx = 14 = 0b1110
        // idx^len  = 0b0001

        // Since idx < len, there must be at least one tree present in len that is not present in
        // idx:
        debug_assert!(diff > 0);

        // Thus this assertion holds, because with at least one bit set, there can't be as many
        // leading zeros as bits:
        debug_assert!(usize::MAX.count_ones() > diff.leading_zeros());

        // ...which in turn means the - 1 will never underflow:
        let height = usize::MAX.count_ones() - diff.leading_zeros() - 1;

        // ...and this will never truncate:
        let height = height as u8;

        // SAFETY: We've proven that height must be in the valid range for a Height
        assert_eq!(Height::MAX as u32, usize::MAX.count_ones() - 1);
        debug_assert!(Height::try_from(height).is_ok());
        let height = unsafe { Height::new_unchecked(height) };

        // Now that we know the height of the tree we want, we need to calculate the index *within*
        // that tree.
        //
        // Again, remember that each bit set in idx corresponds to a tree *prior* to the idx we
        // want. Equally, a 2^i tree has i bits of relevant idx. So it suffices to simply mask off
        // the irrelevant bits:
        let idx_mask = !(usize::MAX << height.get());
        let idx_in_peak = idx & idx_mask;

        Some((height, idx_in_peak))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test_idx_to_containing_height() {
        use std::convert::TryInto;

        macro_rules! t {
            ($len:expr, $idx:expr => $expected:expr) => {
                {
                    let len: usize = $len;
                    let expected: Option<(u8, usize)> = $expected;
                    let expected = expected.map(|(h, i)| (Height::try_from(h).unwrap(), i));
                    assert_eq!(idx_to_containing_height(len.try_into().unwrap(), $idx),
                               expected);
                }
            }
        }

        // Smallest possible:
        t!(0b1, 0 => Some((0, 0)));
        t!(0b1, 1 => None);
        t!(0b1, usize::MAX => None);

        // One tree, of len 2^1 = 2:
        t!(0b10, 0 => Some((1, 0)));
        t!(0b10, 1 => Some((1, 1)));
        t!(0b10, 3 => None);

        // Two trees:
        t!(0b11, 0 => Some((1, 0)));
        t!(0b11, 1 => Some((1, 1)));
        t!(0b11, 2 => Some((0, 0)));
        t!(0b11, 3 => None);

        // Two trees:
        let height_a = 6;
        let height_b = 3;
        let len_a = 1 << height_a;
        let len_b = 1 << height_b;

        // Test every index in the first tree:
        for i in 0 .. len_a {
            t!(len_a + len_b, i => Some((height_a, i)));
        }

        // ...and the second:
        for i in len_a .. len_a + len_b {
            t!(len_a + len_b, i => Some((height_b, i - len_a)));
        }

        // Maximum possible size MMR:
        t!(usize::MAX, 0 => Some((63, 0)));

        let mut idx = (1 << 63) - 1;
        t!(usize::MAX, idx => Some((63, idx)));

        // Adding one gets us into the next biggest tree:
        idx += 1;
        t!(usize::MAX, idx => Some((62, 0)));

        // Biggest possible valid index:
        t!(usize::MAX, usize::MAX - 1 => Some((0, 0)));

        // Biggest possible index:
        t!(usize::MAX, usize::MAX => None);
    }


    #[test]
    fn test_get() {
        let mut mmr = MMR::<u32,Heap>::default();

        assert_eq!(mmr.get(0), None);
        assert_eq!(mmr.get(1), None);
        assert_eq!(mmr.get(usize::MAX), None);

        for i in 0 .. 64 {
            mmr.try_push(i).unwrap();

            for j in 0 ..= i {
                assert_eq!(mmr.get(j as usize).unwrap(), &j);
            }
            assert_eq!(mmr.get(i as usize + 1), None);
        }
    }
}
