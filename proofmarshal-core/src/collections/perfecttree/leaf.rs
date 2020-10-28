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
pub struct Leaf<T, S, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    digest: SyncOnceCell<Digest>,
    sum: SyncOnceCell<S>,
    pub(crate) bag: Bag<T, Z, P>,
}

impl<T, S, Z: Zone> Leaf<T, S, Z> {
    pub fn new_in(value: T, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc,
    {
        Self::new_unchecked(None, None, zone.borrow_mut().alloc(value))
    }
}

impl<T, S, Z, P: Ptr> Leaf<T, S, Z, P> {
    pub fn new_unchecked(digest: Option<Digest>, sum: Option<S>, bag: Bag<T, Z, P>) -> Self {
        Self {
            digest: digest.map(SyncOnceCell::from).unwrap_or_default(),
            sum: sum.map(SyncOnceCell::from).unwrap_or_default(),
            bag
        }
    }

    pub fn try_digest(&self) -> Option<&Digest> {
        self.digest.get()
    }

    pub fn sum(&self) -> S
        where S: MerkleSum<T>
    {
        if let Some(sum) = self.try_sum() {
            sum
        } else {
            self.calc_sum()
        }
    }

    #[inline(never)]
    fn calc_sum(&self) -> S
        where S: MerkleSum<T>
    {
        if let Ok(item) = self.bag.try_get_dirty() {
            let sum = S::from_item(item);
            self.sum.set(sum).ok().expect("calc_sum() called when sum present");
            sum
        } else {
            unreachable!("sum missing, yet value clean")
        }
    }

    pub fn try_sum(&self) -> Option<S>
        where S: 'static + Copy
    {
        let unit: &dyn Any = &();
        if let Some(sum) = unit.downcast_ref::<S>() {
            Some(*sum)
        } else {
            self.sum.get().copied()
        }
    }
}


// ---- hoard impls ------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub enum DecodeLeafBytesError<Sum: fmt::Debug, Bag: fmt::Debug> {
    Sum(Sum),
    Bag(Bag),
}

impl<T, S, Z, P: PtrBlob> Blob for Leaf<T, S, Z, P>
where T: Blob,
      S: Blob,
      Z: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + <S as Blob>::SIZE + <Bag<T, Z, P> as Blob>::SIZE;

    type DecodeBytesError = DecodeLeafBytesError<<S as Blob>::DecodeBytesError, <Bag<T, Z, P> as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> hoard::blob::Bytes<'a, Self> {
        dst.write_struct()
            .write_field(self.digest.get().expect("digest missing"))
            .write_field(self.sum.get().expect("sum missing"))
            .write_field(&self.bag)
            .done()
    }

    fn decode_bytes(blob: hoard::blob::Bytes<'_, Self>) -> std::result::Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();
        let digest = fields.trust_field::<Digest>().into_ok();
        let sum = fields.trust_field::<S>().map_err(DecodeLeafBytesError::Sum)?;
        let bag = fields.trust_field().map_err(DecodeLeafBytesError::Bag)?;
        fields.assert_done();

        Ok(Self {
            digest: digest.into(),
            sum: sum.into(),
            bag,
        }.into())
    }
}

impl<T, S, Z, P: Ptr> Load for Leaf<T, S, Z, P>
where T: Load,
      S: Blob,
      Z: Zone,
{
    type Blob = Leaf<T::Blob, S, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        Self {
            digest: blob.digest,
            sum: blob.sum,
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
        let leaf = Leaf::<u8, (), _>::new_in(42u8, Heap);
        assert_eq!(leaf.try_sum(), Some(()));
        assert_eq!(leaf.sum(), ());
    }
}
