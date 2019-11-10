use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use super::*;

use crate::impls::ScalarEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    raw: NonZeroU64,
}

impl<'p> Offset<'p> {
    pub const MAX: u64 = (1 << 62) - 1;

    pub fn new(offset: u64) -> Option<Self> {
        if offset <= Self::MAX {
            Some(Self {
                marker: PhantomData,
                raw: NonZeroU64::new((offset << 1) | 1).unwrap(),
            })
        } else {
            None
        }
    }

    pub fn persist(self) -> Offset<'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    pub fn get(self) -> u64 {
        self.raw.get() >> 1
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
        dst.write_bytes(&self.value.raw.get().to_le_bytes())?
           .done()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p>(Offset<'p>);

#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'p> {
    Offset(Offset<'p>),
    Ptr(NonNull<u16>),
}

impl<'p> OffsetMut<'p> {
    pub fn from_offset(offset: Offset<'p>) -> Self {
        Self(offset)
    }

    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

    pub fn kind(&self) -> Kind<'p> {
        match self.0.raw.get() & 1 {
            1 => Kind::Offset(self.0),
            0 => Kind::Ptr(unsafe {
                let raw = self.0.raw.get();
                NonNull::new_unchecked(raw as usize as *mut u16)
            }),
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for OffsetMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.kind(), f)
    }
}
