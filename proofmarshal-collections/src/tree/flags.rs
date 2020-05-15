use std::sync::atomic::AtomicU8;
use std::mem;

use thiserror::Error;

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

#[derive(Debug, Error)]
#[error("invalid flags")]
pub struct LoadFlagsError(u8);

impl Load for Flags {
    type Error = LoadFlagsError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

impl<R> Saved<R> for Flags {
    type Saved = Self;
}
impl<Q, R> Save<'_, Q, R> for Flags {
    type State = ();

    fn init_save_state(&self) -> Self::State {}

    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut Self::State, dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        <Self as Save<Q,R>>::encode_blob(self, state, dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&[0])?
           .done()
    }
}
impl Primitive for Flags {}

impl From<Flags> for AtomicU8 {
    #[inline(always)]
    fn from(flags: Flags) -> Self {
        flags.bits.into()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
