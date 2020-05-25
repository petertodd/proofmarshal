use std::sync::atomic::AtomicU8;
use std::mem;

use thiserror::Error;

use hoard::blob::*;
use hoard::save::*;
use hoard::load::*;
use hoard::primitive::*;

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

#[derive(Debug, Error)]
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

impl<Z> Decode<Z> for Flags {
    fn decode_blob(blob: hoard::load::BlobDecoder<Z, Self>) -> Self {
        blob.to_value().clone()
    }
}

unsafe impl Persist for Flags {}

impl<R> Encoded<R> for Flags {
    type Encoded = Self;
}

impl<Q, R> Encode<'_, Q, R> for Flags {
    type State = ();
    fn init_encode_state(&self) -> () {}

    fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_primitive(&[self.bits])?
           .done()
    }
}

impl Primitive for Flags {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
