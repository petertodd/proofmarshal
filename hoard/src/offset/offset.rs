use std::convert::TryFrom;
use std::marker::PhantomData;
use std::cmp;

use crate::blob::{Blob, Bytes, BytesUninit};
use crate::primitive::Primitive;
use crate::pointee::Pointee;
use crate::ptr::PtrBlob;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(u64);

impl Primitive for Offset {
    const BLOB_SIZE: usize = 8;
    type DecodeBytesError = !;

    fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let buf = TryFrom::try_from(&blob[..]).unwrap();
        Ok(Self(u64::from_le_bytes(buf)))
    }

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&self.0.to_le_bytes())
    }
}

impl PtrBlob for Offset {
}

impl Offset {
    pub const fn new(offset: u64) -> Self {
        Self(offset)
    }

    pub fn dangling() -> Self {
        Offset(u64::max_value())
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<usize> for Offset {
    fn from(offset: usize) -> Self {
        Offset(offset as u64)
    }
}

impl From<u64> for Offset {
    fn from(offset: u64) -> Self {
        Offset(offset)
    }
}

impl From<Offset> for u64 {
    fn from(offset: Offset) -> Self {
        offset.0
    }
}

impl cmp::PartialEq<u64> for Offset {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl cmp::PartialEq<Offset> for u64 {
    fn eq(&self, other: &Offset) -> bool {
        *self == other.0
    }
}
