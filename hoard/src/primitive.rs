//! Marshalling of primitive types without internal pointers.

use std::num;
use std::convert::TryFrom;

use crate::pointee::Pointee;
use crate::load::*;
use crate::blob::*;

use leint::Le;

pub trait Primitive : Decode<()> + EncodeBlob<()> {
    fn encode_blob_bytes(&self) -> Vec<u8> {
        let mut poll = self.init_encode_blob(&mut DummyBlobSaver).into_ok();
        poll.encode_blob_poll(&mut DummyBlobSaver).into_ok();
        let mut r = vec![];
        poll.encode_blob(&mut r).into_ok();
        r
    }

    fn try_decode_blob_bytes(src: &[u8]) -> Result<Self, <Self as ValidateBlob<padding::CheckPadding>>::Error>
        where Self: Sized
    {
        let blob = Blob::<Self>::try_from(src).expect("incorrect size");
        let valid_blob = Self::validate_blob(blob, padding::CheckPadding)?;
        Ok(Self::decode_blob(valid_blob, &()))
    }
}

struct DummyBlobSaver;

impl BlobSaver for DummyBlobSaver {
    type DstZone = ();
    type SrcPtr = !;
    type Error = !;
    type Write = !;

    unsafe fn try_get_dirty<T>(&self, ptr: &Self::SrcPtr, metadata: T::Metadata)
        -> Result<Result<&T, <Self::DstZone as BlobZone>::BlobPtr>,
                  Self::Error>
        where T: ?Sized + Pointee,
              Self::DstZone: BlobZone,
    {
        unreachable!()
    }

    fn alloc_blob<F>(&mut self, size: usize, f: F) -> Result<<Self::DstZone as BlobZone>::BlobPtr, Self::Error>
        where F: FnOnce(&mut Self::Write) -> Result<(), <Self::Write as Write>::Error>,
              Self::DstZone: BlobZone,
    {
        unreachable!()
    }

}


impl<T: Primitive, const N: usize> Primitive for [T; N] {}

impl<T: Primitive> Primitive for Option<T> {}

macro_rules! impl_primitive {
    ($( $t:ty, )+) => {$(
        impl Primitive for $t {}
    )+}
}

impl_primitive! {
    !, (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    //u16, u32, u64, u128,
    //i16, i32, i64, i128,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

pub fn test_option_decode(src: &[u8;2]) -> Option<Option<bool>>
{
    Primitive::try_decode_blob_bytes(src).ok()
}

pub fn test_array_decode(src: &[u8;4])
    -> Result<[bool; 4],
              crate::blob::impls::array::ValidateArrayError<crate::blob::impls::scalars::ValidateBoolError, 4>>
{
    Primitive::try_decode_blob_bytes(src)
}

pub fn test_array_decode2(src: &[u8;2])
    -> Option<[bool; 2]>
{
    Primitive::try_decode_blob_bytes(src)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option() {
        assert_eq!(Some(42u8).encode_blob_bytes(),
                   &[1, 42]);

        assert_eq!(<Option<u8> as Primitive>::try_decode_blob_bytes(&[1,42]).unwrap(),
                   Some(42u8));

        assert_eq!(<Option<bool> as Primitive>::try_decode_blob_bytes(&[1,1]).unwrap(),
                   Some(true));
    }

    #[test]
    fn test_ints() {
        /*
        assert_eq!(0x12345678_u32.encode_blob_bytes(),
                   &[0x78, 0x56, 0x34, 0x12]);

        assert_eq!(<u32 as Primitive>::try_decode_blob_bytes(&[0x78, 0x56, 0x34, 0x12]),
                   Ok(0x12345678));
        */
    }
}
