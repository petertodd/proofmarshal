use crate::blob::{Blob, Bytes, BytesUninit, ValidBytes};
use crate::owned::Ref;
use crate::load::Load;

pub trait Scalar : Copy + core::fmt::Debug + Eq + Ord + core::hash::Hash + Send + Sync {
    const SIZE: usize;

    type Error;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> ValidBytes<'a, Self>;

    fn validate_blob_bytes<'a>(blob: Bytes<'a, Self>) -> Result<ValidBytes<'a, Self>, Self::Error>;
    fn decode_blob_bytes(blob: ValidBytes<'_, Self>) -> Self;

    fn deref_blob_bytes<'a>(blob: ValidBytes<'_, Self>) -> Ref<'a, Self> {
        Ref::Owned(Self::decode_blob_bytes(blob))
    }
}

impl<T: Scalar

impl<T: Scalar> Load for T {
    type Zone = ();
    type Blob = Self;
}


impl Scalar for ! {
    const SIZE: usize = 0;

    type Error = !;

    fn validate_blob_bytes<'a>(blob: Bytes<'a, Self>) -> Result<ValidBytes<'a, Self>, Self::Error> {
        unsafe { Ok(blob.assume_valid()) }
    }

    fn decode_blob_bytes(_bytes: ValidBytes<'_, Self>) -> Self {
        panic!()
    }

    fn encode_blob_bytes<'a>(&self, _dst: BytesUninit<'a, Self>) -> ValidBytes<'a, Self> {
        match *self {}
    }
}

impl Scalar for () {
    const SIZE: usize = 0;

    type Error = !;

    fn validate_blob_bytes<'a>(blob: Bytes<'a, Self>) -> Result<ValidBytes<'a, Self>, Self::Error> {
        unsafe { Ok(blob.assume_valid()) }
    }

    fn decode_blob_bytes(_bytes: ValidBytes<'_, Self>) -> Self {
        ()
    }

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> ValidBytes<'a, Self> {
        let bytes = dst.write_bytes(&[]);
        unsafe { ValidBytes::new_unchecked(bytes) }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct DecodeBoolError;

impl Scalar for bool {
    const SIZE: usize = 1;

    type Error = DecodeBoolError;

    fn validate_blob_bytes<'a>(blob: Bytes<'a, Self>) -> Result<ValidBytes<'a, Self>, Self::Error> {
        match blob[0] {
            0 | 1 => unsafe { Ok(blob.assume_valid())},
            _ => Err(DecodeBoolError),
        }
    }

    fn decode_blob_bytes(blob: ValidBytes<'_, Self>) -> Self {
        match blob[0] {
            0 => false,
            1 => true,
            _ => unreachable!(),
        }
    }

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> ValidBytes<'a, Self> {
        let bytes = dst.write_bytes(&[*self as u8]);
        unsafe { ValidBytes::new_unchecked(bytes) }
    }
}
