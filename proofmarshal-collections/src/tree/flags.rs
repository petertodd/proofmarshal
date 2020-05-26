use std::sync::atomic::AtomicU8;
use std::mem;

use thiserror::Error;

use hoard::blob::*;
use hoard::save::*;
use hoard::load::*;
use hoard::primitive::*;
use hoard::ptr::Ptr;

bitflags::bitflags! {
    pub struct Flags: u8 {
        const DIGEST_DIRTY  = 0b0001;
        const DIGEST_LOCKED = 0b0010;
        const SUM_DIRTY     = 0b0100;
        const SUM_LOCKED    = 0b1000;
    }
}

impl From<Flags> for AtomicU8 {
    #[inline(always)]
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
#[error("invalid flags: {0}")]
pub struct ValidateFlagsBlobError(u8);

impl ValidateBlob for Flags {
    type Error = ValidateFlagsBlobError;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.field_bytes(1)[0] {
            0 => unsafe { Ok(blob.finish()) },
            x => Err(ValidateFlagsBlobError(x)),
        }
    }
}

impl<Q: Ptr> Decode<Q> for Flags {
    fn decode_blob(blob: hoard::load::BlobDecoder<Q, Self>) -> Self {
        blob.to_value().clone()
    }
}

unsafe impl Persist for Flags {}

impl<Q, R> Encode<Q, R> for Flags {
    type EncodePoll = u8;

    fn init_encode(&self, _: &impl SavePtr) -> u8 {
        assert!(self.is_empty(), "some flags set: {:?}", self);
        0
    }
}

impl Primitive for Flags {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_marshalling() {
        assert_eq!(Flags::try_decode_blob_bytes(&[0]),
                   Ok(Flags::empty()));

        // having any flags set at all is invalid
        for i in 1 ..= 255 {
            assert_eq!(Flags::try_decode_blob_bytes(&[i]),
                       Err(ValidateFlagsBlobError(i)));
        }

        assert_eq!(Flags::empty().encode_blob_bytes(), &[0]);
    }

    #[test]
    #[should_panic]
    fn flags_marshalling_panics_if_not_empty() {
        Flags::DIGEST_DIRTY.encode_blob_bytes();
    }
}
