//! Merkle Mountain Ranges: the merkelized equivalent of a `Vec`.

use std::borrow::{Borrow, BorrowMut};
use std::cmp;
use std::convert::TryFrom;
use std::error;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::DerefMut;
use std::ptr;

use thiserror::Error;

use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::bag::Bag;
use hoard::primitive::Primitive;
use hoard::owned::{IntoOwned, Take, Ref, RefOwn};
use hoard::pointee::Pointee;
use hoard::ptr::{Get, GetMut, Ptr, PtrBlob, Zone, AsZone};
use hoard::load::{Load, LoadRef, MaybeValid};
use hoard::save::{Save, SavePoll, Saver};

use crate::commit::{
    Commit, Digest,
    sha256::Sha256Digest,
};
use crate::collections::leaf::Leaf;
use crate::collections::length::*;
use crate::collections::height::Height;
use crate::collections::perfecttree::PerfectTree;

pub mod peaktree;
use self::peaktree::{PeakTree, PeakTreeDyn, DecodePeakTreeBytesError, DecodePeakTreeDynBytesError, PeakTreeSavePoll};

#[derive(Debug)]
pub struct MMR<T, P: Ptr, D: Digest = Sha256Digest> {
    peaks: Option<PeakTree<T, P, D>>,
}

impl<T: Commit, P: Ptr, D: Digest> Commit for MMR<T, P, D> {
    type Commitment = MMR<T::Commitment, (), D>;

    fn to_commitment(&self) -> Self::Commitment {
        /*
        MMR {
            peaks: self.peaks.to_commitment(),
        }
        */ todo!()
    }
}

impl<T, P: Ptr, D: Digest> Default for MMR<T, P, D>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, P: Ptr, D: Digest> MMR<T, P, D> {
    pub fn new() -> Self {
        Self {
            peaks: None,
        }
    }

    pub fn len(&self) -> Length {
        self.peaks.as_ref()
            .map(|peaks| {
                peaks.len().into()
            }).unwrap_or(Length(0))
    }

    pub fn peaks(&self) -> Option<&PeakTree<T, P, D>> {
        self.peaks.as_ref()
    }

    pub fn peaks_mut(&mut self) -> Option<&mut PeakTree<T, P, D>> {
        self.peaks.as_mut()
    }
}

impl<T, P: Ptr, D: Digest> MMR<T, P, D>
where T: Load,
      P::Zone: AsZone<T::Zone>
{
    pub fn try_push(&mut self, value: T) -> Result<(), T>
        where P: GetMut + Default
    {
        if self.len() < Length::MAX {
            let leaf = Leaf::new(value);
            match self.try_push_leaf(leaf) {
                Ok(()) => Ok(()),
                Err(_overflow) => unreachable!("overflow condition already checked"),
            }
        } else {
            Err(value)
        }
    }

    pub fn try_push_leaf(&mut self, leaf: Leaf<T, P, D>) -> Result<(), Leaf<T, P, D>>
        where P: GetMut + Default
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
        where P: Get,
    {
        self.get_leaf(idx).map(|leaf| {
            match leaf {
                Ref::Borrowed(leaf) => leaf.get(),
                Ref::Owned(leaf) => Ref::Owned(leaf.take()),
            }
        })
    }

    pub fn into_get(self, idx: usize) -> Option<T>
        where P: Get,
    {
        self.into_get_leaf(idx).map(|leaf| leaf.take())
    }

    pub fn get_leaf(&self, idx: usize) -> Option<Ref<Leaf<T, P, D>>>
        where P: Get
    {
        match &self.peaks {
            Some(peaks) => {
                if let Some((height, idx_in_peak)) = idx_to_containing_height(peaks.len(), idx) {
                    peaks.get(height).and_then(|peak| {
                        match peak {
                            Ref::Borrowed(peak) => peak.get_leaf(idx_in_peak),
                            Ref::Owned(peak) => peak.into_get_leaf(idx_in_peak)
                                                    .map(Ref::Owned)
                        }
                    })
                } else {
                    None
                }
            },
            None => None,
        }
    }

    pub fn into_get_leaf(self, idx: usize) -> Option<Leaf<T, P, D>>
        where P: Get
    {
        match self.peaks {
            Some(peaks) => {
                if let Some((height, idx_in_peak)) = idx_to_containing_height(peaks.len(), idx) {
                    peaks.into_get(height).and_then(|peak| peak.into_get_leaf(idx_in_peak))
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


#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeMMRBytesError<Peaks: error::Error, Len: error::Error> {
    Peaks(Peaks),
    Len(Len),
    NonZeroPadding,
}

impl<T, P: Ptr, D: Digest> Blob for MMR<T, P, D>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <PeakTree<T, P, D> as Blob>::SIZE;
    type DecodeBytesError = DecodeMMRBytesError<<PeakTreeDyn<T, P, D> as BlobDyn>::DecodeBytesError,
                                                <Length as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        if let Some(peaks) = &self.peaks {
            dst.write_struct()
               .write_field(peaks)
               .done()
        } else {
            dst.write_struct()
               .write_padding(<PeakTree<T, P, D> as Blob>::SIZE - <Length as Blob>::SIZE)
               .write_field(&Length::ZERO)
               .done()
        }
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();

        let peaks = match fields.trust_field::<PeakTree<T, P, D>>() {
            Ok(peaks) => Ok(Some(peaks)),
            Err(DecodePeakTreeBytesError::Raw(raw)) => Err(DecodeMMRBytesError::Peaks(DecodePeakTreeDynBytesError(raw))),
            Err(DecodePeakTreeBytesError::NonZeroLength(err)) if err.0 == 0 => {
                // FIXME: check for non-zero padding
                Ok(None)
            },
            Err(DecodePeakTreeBytesError::NonZeroLength(_err)) => Err(DecodeMMRBytesError::Len(LengthError)),
        }?;

        fields.assert_done();
        Ok(Self { peaks }.into())
    }
}

impl<T, P: Ptr, D: Digest> Load for MMR<T, P, D>
where T: Load
{
    type Blob = MMR<T::Blob, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        Self {
            peaks: Load::load(blob.peaks, zone),
        }
    }
}

#[doc(hidden)]
pub struct MMRSavePoll<Q: PtrBlob, T: Save<Q>, P: Ptr, D: Digest> {
    peaks: Option<PeakTreeSavePoll<Q, T, P, D>>,
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> SavePoll for MMRSavePoll<Q, T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = MMR<T::DstBlob, Q, D>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        match &mut self.peaks {
            None => Ok(()),
            Some(peaks) => peaks.save_poll(saver),
        }
    }

    fn encode_blob(&self) -> Self::DstBlob {
        MMR {
            peaks: self.peaks.as_ref().map(SavePoll::encode_blob),
        }
    }
}

impl<Q: PtrBlob, T, P: Ptr, D: Digest> Save<Q> for MMR<T, P, D>
where T: Commit + Save<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = MMR<T::DstBlob, Q, D>;
    type SavePoll = MMRSavePoll<Q, T, P, D>;

    fn init_save(&self) -> Self::SavePoll {
        MMRSavePoll {
            peaks: self.peaks.as_ref().map(Save::init_save),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use hoard::{
        ptr::{
            Heap,
            PtrClean,
            key::{
                Key, KeyMut, Map,
                offset::OffsetSaver,
            },
        },
    };

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
    fn heap_get() {
        let mut mmr = MMR::<u32,Heap>::new();

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

    #[test]
    fn save() {
        let mut mmr = MMR::<u8, Heap>::new();

        #[track_caller]
        fn t(mmr: &MMR<u8, Heap>, expected_offset: u64, expected_buf: &[u8]) {
            let saver = OffsetSaver::new(&[][..]);
            let (offset, buf) = saver.try_save(mmr).unwrap();
            assert_eq!(offset, expected_offset);
            assert_eq!(buf, expected_buf);
        }

        t(&mmr, 0, &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]);

        mmr.try_push(42).unwrap();
        t(&mmr, 1, &[
            42,
            42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0
        ]);

        mmr.try_push(43).unwrap();
        t(&mmr, 82, &[
            42, 43, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 250, 74, 74, 79, 147, 58, 207, 22, 6, 235, 253, 179, 116, 242, 232, 247, 23, 215, 116, 250, 221, 195, 13, 100, 238, 143, 228, 191, 182, 184, 237, 154, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0
        ]);

        mmr.try_push(44).unwrap();
        t(&mmr, 163, &[
            42, 43, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 44, 250, 74, 74, 79, 147, 58, 207, 22, 6, 235, 253, 179, 116, 242, 232, 247, 23, 215, 116, 250, 221, 195, 13, 100, 238, 143, 228, 191, 182, 184, 237, 154, 2, 0, 0, 0, 0, 0, 0, 0, 44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 82, 0, 0, 0, 0, 0, 0, 0, 138, 125, 6, 251, 217, 239, 106, 103, 2, 109, 242, 95, 16, 143, 157, 213, 3, 198, 231, 236, 185, 164, 188, 23, 157, 97, 151, 110, 147, 47, 235, 190, 83, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0
        ]);

        mmr.try_push(45).unwrap();
        t(&mmr, 244, &[
            42, 43, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 44, 45, 44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 82, 0, 0, 0, 0, 0, 0, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 83, 0, 0, 0, 0, 0, 0, 0, 250, 74, 74, 79, 147, 58, 207, 22, 6, 235, 253, 179, 116, 242, 232, 247, 23, 215, 116, 250, 221, 195, 13, 100, 238, 143, 228, 191, 182, 184, 237, 154, 2, 0, 0, 0, 0, 0, 0, 0, 164, 244, 200, 22, 32, 194, 203, 252, 168, 252, 91, 233, 88, 222, 38, 115, 86, 146, 146, 86, 1, 152, 121, 190, 253, 18, 48, 244, 155, 44, 213, 159, 84, 0, 0, 0, 0, 0, 0, 0, 79, 64, 109, 199, 230, 180, 200, 195, 102, 189, 161, 26, 69, 87, 71, 112, 6, 153, 86, 6, 222, 176, 115, 211, 60, 242, 180, 8, 244, 74, 221, 110, 164, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0
        ]);
    }

    #[test]
    fn save_then_get() {
        let mut mmr = MMR::<u16, Heap>::new();

        for i in 0 .. 255 {
            mmr.try_push(i).unwrap();
        }

        let saver = OffsetSaver::new(&[][..]);
        let (offset, buf) = saver.try_save(&mmr).unwrap();
        assert_eq!(offset, 20830);

        let map: &[u8] = &buf;
        let key = Key::<[u8]>::from_blob(offset, &map);

        let bag: Bag<MMR<u16, Key<[u8]>>, _> = unsafe { Bag::from_raw_parts(key, ()) };

        let mmr = bag.get();
        assert_eq!(mmr.len(), 255);

        for i in 0u16 .. 255 {
            assert_eq!(mmr.get(i as usize).unwrap(), &i);
        }

        let keymut = KeyMut::Key(key);
        let mut bag: Bag<MMR<u16, KeyMut<[u8]>>, _> = unsafe { Bag::from_raw_parts(keymut, ()) };
        let mmr = bag.get_mut();

        mmr.try_push(255).unwrap();
        assert_eq!(mmr.len(), 256);

        for i in 256 .. 511 {
            mmr.try_push(i).unwrap();
        }

        // verify in dirty state
        for i in 0 .. 511 {
            assert_eq!(mmr.get(i as usize).unwrap(), &i);
        }

        let saver = OffsetSaver::new(map);
        let (offset, buf) = saver.try_save(mmr).unwrap();
        assert_eq!(offset, 41822);

        let map: &[u8] = &buf;
        let key = Key::<[u8]>::from_blob(offset, &map);
        let bag: Bag<MMR<u16, Key<[u8]>>, _> = unsafe { Bag::from_raw_parts(key, ()) };

        // verify in clean state
        for i in 0 .. 511 {
            assert_eq!(bag.get().get(i as usize).unwrap(), &i);
        }
    }
}
