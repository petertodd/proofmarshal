use std::cell::Cell;
use std::ops::{Deref, DerefMut};

use thiserror::Error;

use hoard::prelude::*;

use hoard::blob::{BlobDyn, Bytes, BytesUninit};
use hoard::owned::Take;
use hoard::ptr::PtrBlob;
use hoard::save::{Save, SaveRef, SavePoll, Saver};

use crate::commit::{
    Commit, CommitRef, HashCommit,
    Digest,
    Sha256Digest,
};

/// A `Bag` whose contents have been hashed.
#[derive(Debug)]
pub struct HashBag<T: ?Sized + Pointee, P: Ptr, D: Digest = Sha256Digest> {
    digest: Cell<Option<D>>,
    bag: Bag<T, P>,
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> HashBag<T, P, D> {
    /// Creates a new `HashBag` from a value.
    pub fn new(value: impl Take<T>) -> Self
        where P: Default
    {
        Self::new_unchecked(None, P::alloc(value))
    }

    /// Creates a new `HashBag`, without checking digest validity or availability.
    ///
    /// This function is *not* marked unsafe, as digests aren't related to memory safety.
    fn new_unchecked(digest: Option<D>, bag: Bag<T, P>) -> Self {
        Self {
            digest: digest.into(),
            bag,
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> From<HashBag<T, P, D>> for Bag<T, P> {
    fn from(hashbag: HashBag<T, P, D>) -> Bag<T, P> {
        hashbag.bag
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> HashBag<T, P, D>
where T: LoadRef,
      P::Zone: AsZone<T::Zone>,
{
    pub fn get_mut<'a>(&'a mut self) -> &'a mut T
        where P: GetMut
    {
        let r = self.bag.get_mut();
        self.digest.set(None);
        r
    }

    pub fn try_get_mut<'a>(&'a mut self) -> Result<&'a mut T, P::Error>
        where P: TryGetMut
    {
        let r = self.bag.try_get_mut()?;
        self.digest.set(None);
        Ok(r)
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> HashBag<T, P, D>
where T: CommitRef,
{
    pub fn target_hash_commit(&self) -> HashCommit<T::CommitmentDyn, D> {
        self.try_target_hash_commit()
            .unwrap_or_else(|| self.calc_target_hash_commit())
    }

    fn calc_target_hash_commit(&self) -> HashCommit<T::CommitmentDyn, D> {
        let target: &T = self.bag.try_get_dirty().ok().expect("digest missing yet bag clean");
        let commit = HashCommit::new(target);
        self.digest.set(Some(commit.digest()));
        commit
    }

    pub fn try_target_hash_commit(&self) -> Option<HashCommit<T::CommitmentDyn, D>> {
        self.digest.get().map(HashCommit::from_digest)
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> Deref for HashBag<T, P, D> {
    type Target = Bag<T, P>;

    fn deref(&self) -> &Self::Target {
        &self.bag
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeHashBagBytesError<Digest: std::error::Error, Bag: std::error::Error> {
    Digest(Digest),
    Bag(Bag),
}

impl<T: ?Sized + Pointee, P: PtrBlob, D: Digest> Blob for HashBag<T, P, D>
where T: BlobDyn,
{
    const SIZE: usize = <D as Blob>::SIZE + <Bag<T, P> as Blob>::SIZE;
    type DecodeBytesError = DecodeHashBagBytesError<<D as Blob>::DecodeBytesError, <Bag<T, P> as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.digest.get().expect("digest missing"))
           .write_field(&self.bag)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let digest = fields.trust_field().map_err(DecodeHashBagBytesError::Digest)?;
        let bag = fields.trust_field().map_err(DecodeHashBagBytesError::Bag)?;
        fields.assert_done();
        Ok(Self::new_unchecked(Some(digest), bag).into())
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> Commit for HashBag<T, P, D>
where T: CommitRef
{
    type Commitment = HashBag<T::CommitmentDyn, (), D>;

    fn to_commitment(&self) -> Self::Commitment {
        todo!()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, D: Digest> Load for HashBag<T, P, D>
where T: LoadRef
{
    type Blob = HashBag<T::BlobDyn, P::Blob, D>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let digest = blob.digest.get().expect("digest missing");
        Self::new_unchecked(Some(digest), Bag::load(blob.bag, zone))
    }
}

#[doc(hidden)]
pub struct HashBagSavePoll<Q: PtrBlob, T: ?Sized + SaveRef<Q>, P: Ptr, D: Digest>
where P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    digest: D,
    bag_poll: <Bag<T, P> as Save<Q>>::SavePoll,
}

impl<Q: PtrBlob, T: ?Sized + SaveRef<Q>, P: Ptr, D: Digest> SavePoll for HashBagSavePoll<Q, T, P, D>
where P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type SrcPtr = P::Clean;
    type DstPtr = Q;
    type DstBlob = HashBag<T::DstBlob, Q, D>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>,
    {
        self.bag_poll.save_poll(saver)
    }

    fn encode_blob(&self) -> Self::DstBlob {
        HashBag::new_unchecked(
            Some(self.digest),
            self.bag_poll.encode_blob(),
        )
    }
}

impl<Q: PtrBlob, T: ?Sized, P: Ptr, D: Digest> Save<Q> for HashBag<T, P, D>
where T: CommitRef + SaveRef<Q>,
      P::Zone: AsZone<T::Zone>,
      P::Clean: From<<T::Ptr as Ptr>::Clean>,
{
    type DstBlob = HashBag<T::DstBlob, Q, D>;
    type SavePoll = HashBagSavePoll<Q, T, P, D>;

    fn init_save(&self) -> Self::SavePoll {
        HashBagSavePoll {
            digest: self.target_hash_commit().digest(),
            bag_poll: self.bag.init_save(),
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;

    use hoard::prelude::*;

    #[test]
    fn target_hash_commit() {
        let bag = HashBag::<u8, Heap>::new(42u8);
        assert_eq!(bag.target_hash_commit(),
                   HashCommit::new(&42u8));
    }
}
