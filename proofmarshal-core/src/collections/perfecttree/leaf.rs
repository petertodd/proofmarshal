use std::any::Any;
use std::borrow::{Borrow, BorrowMut};
use std::convert::TryFrom;
use std::fmt;
use std::lazy::SyncOnceCell;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops::DerefMut;
use std::ptr;

use thiserror::Error;

use hoard::primitive::Primitive;
use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::load::{MaybeValid, Load, LoadRef};
use hoard::save::{SaveDirty, SaveDirtyPoll, SaveDirtyRef, SaveDirtyRefPoll, BlobSaver};
use hoard::zone::{Alloc, AsZone, Zone, Get, GetMut, Ptr, PtrConst, PtrBlob, FromPtr};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, Own, Ref};
use hoard::bag::Bag;

use crate::collections::merklesum::MerkleSum;
use crate::commit::{Commit, WriteVerbatim, Digest};

#[derive(Debug)]
pub struct Leaf<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    digest: SyncOnceCell<Digest>,
    pub(crate) bag: Bag<T, Z, P>,
}

impl<T, Z: Zone> Leaf<T, Z> {
    pub fn new_in(value: T, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc,
    {
        Self::new_unchecked(None, zone.borrow_mut().alloc(value))
    }
}

impl<T, Z: Zone> Leaf<T, Z>
where T: Load
{
    pub fn get(&self) -> Result<Ref<T>, Z::Error>
        where Z: Get + AsZone<T::Zone>
    {
        self.bag.get()
    }

    pub fn get_mut(&mut self) -> Result<&mut T, Z::Error>
        where Z: GetMut + AsZone<T::Zone>
    {
        self.bag.get_mut()
    }
}

impl<T, Z, P: Ptr> Leaf<T, Z, P> {
    pub fn new_unchecked(digest: Option<Digest>, bag: Bag<T, Z, P>) -> Self {
        Self {
            digest: digest.map(SyncOnceCell::from).unwrap_or_default(),
            bag
        }
    }

    pub fn try_digest(&self) -> Option<&Digest> {
        self.digest.get()
    }
}

// ---- hoard impls ------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeLeafBytesError<Bag: fmt::Debug> {
    Bag(Bag),
}

impl<T, Z, P: PtrBlob> Blob for Leaf<T, Z, P>
where T: Blob,
      Z: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + <Bag<T, Z, P> as Blob>::SIZE;

    type DecodeBytesError = DecodeLeafBytesError<<Bag<T, Z, P> as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> {
        dst.write_struct()
            .write_field(self.digest.get().expect("digest missing"))
            .write_field(&self.bag)
            .done()
    }

    fn decode_bytes(blob: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();
        let digest = fields.trust_field::<Digest>().into_ok();
        let bag = fields.trust_field().map_err(DecodeLeafBytesError::Bag)?;
        fields.assert_done();

        Ok(Self {
            digest: digest.into(),
            bag,
        }.into())
    }
}

impl<T, Z, P: Ptr> Load for Leaf<T, Z, P>
where T: Load,
      Z: Zone,
{
    type Blob = Leaf<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        Self {
            digest: blob.digest,
            bag: Bag::load(blob.bag, zone),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test_sum() {
        let _leaf = Leaf::<u8, _>::new_in(42u8, Heap);
    }
}
