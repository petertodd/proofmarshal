use core::cmp;
use core::convert::TryFrom;
use core::fmt;
use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use super::*;

use crate::impls::ScalarEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    raw: u64,
}

impl<'p> Offset<'p> {
    pub unsafe fn new_unchecked(n: usize) -> Self {
        Self {
            marker: PhantomData,
            raw: n as u64,
        }
    }

    pub fn persist(&self) -> Offset<'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }
}

impl<Z: Zone> Encode<Z> for Offset<'_> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(8);

    type Encode = ScalarEncoder<Self, Z>;
    fn encode(self) -> Self::Encode {
        self.into()
    }
}

impl<'p, Z: Zone> EncodePoll for ScalarEncoder<Offset<'p>, Z> {
    type Zone = Z;
    type Target = Offset<'p>;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&self.value.raw.to_le_bytes())?
           .done()
    }
}
