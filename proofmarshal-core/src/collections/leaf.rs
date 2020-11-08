use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::mem::{self, ManuallyDrop};
use std::ptr;

use thiserror::Error;

use hoard::primitive::Primitive;
use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::load::{MaybeValid, Load, LoadRef};
use hoard::save::{Save, SavePoll, Saver};
use hoard::ptr::{AsZone, Zone, Get, GetMut, Ptr, PtrClean, PtrBlob};
use hoard::pointee::Pointee;
use hoard::owned::{IntoOwned, Take, RefOwn, Ref};
use hoard::bag::Bag;

use crate::commit::{Commit, WriteVerbatim, Digest};

use super::raw;

/// Leaf node in a tree.
#[repr(transparent)]
pub struct Leaf<T, P: Ptr> {
    raw: ManuallyDrop<raw::Node<T, P>>,
}

impl<T, P: Ptr> Drop for Leaf<T, P> {
    fn drop(&mut self) {
        unsafe {
            self.raw.ptr.dealloc::<T>(())
        }
    }
}

impl<T, P: Ptr> Leaf<T, P> {
    pub fn new(value: T) -> Self
        where P: Default,
    {
        Self::new_unchecked(None, P::alloc(value))
    }
}

impl<T, P: Ptr> Leaf<T, P> {
    pub fn new_unchecked(digest: Option<Digest>, bag: Bag<T, P>) -> Self {
        let (ptr, ()) = bag.into_raw_parts();
        let raw = raw::Node::new(digest, ptr);

        unsafe {
            Self::from_raw(raw)
        }
    }

    pub unsafe fn from_raw(raw: raw::Node<T, P>) -> Self {
        Self {
            raw: ManuallyDrop::new(raw),
        }
    }

    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, P>) -> &Self {
        &*(raw as *const _ as *const _)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, P>) -> &mut Self {
        &mut *(raw as *mut _ as *mut _)
    }

    pub fn into_raw(self) -> raw::Node<T, P> {
        let this = ManuallyDrop::new(self);
        unsafe {
            ptr::read(&*this.raw)
        }
    }

    /*
    pub fn digest(&self) -> Digest<T::Committed>
        where T: Commit
    {
        self.try_digest()
            .map(|digest| digest.cast())
            .unwrap_or_else(|| self.calc_digest())
    }

    fn calc_digest(&self) -> Digest<T::Committed>
        where T: Commit
    {
        let value = self.try_get_dirty()
                        .ok().expect("digest missing yet leaf value clean");
        let digest = value.commit();
        self.raw.set_digest(digest.cast());
        digest
    }

    pub fn try_digest(&self) -> Option<Digest> {
        self.raw.digest()
    }
    */
}

impl<T, P: Ptr> Leaf<T, P>
where T: Load,
      P::Zone: AsZone<T::Zone>,
{
    pub fn get(&self) -> Ref<T>
        where P: Get
    {
        unsafe {
            self.raw.get::<T>(T::sized_metadata())
                    .trust()
        }
    }

    pub fn get_mut(&mut self) -> &mut T
        where P: GetMut
    {
        unsafe {
            self.raw.get_mut::<T>(T::sized_metadata())
                    .trust()
        }
    }

    pub fn take(self) -> T
        where P: Get
    {
        let raw = self.into_raw();
        unsafe {
            raw.take::<T>(T::sized_metadata())
               .trust()
        }
    }
}

impl<T, P: Ptr> Leaf<T, P> {
    pub fn try_get_dirty(&self) -> Result<&T, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(())
                    .map(MaybeValid::trust)
        }
    }
}

impl<T, P: Ptr> fmt::Debug for Leaf<T, P>
where T: fmt::Debug, P: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Leaf")
            .field("digest", &self.raw.digest())
            .field("ptr", &self.try_get_dirty().map_err(P::from_clean))
            .finish()
    }
}

/*
impl<T, P: Ptr> Commit for Leaf<T, P>
where T: Commit
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN;
    type Committed = T::Committed;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.digest().as_bytes())
    }
}
*/

// ---- hoard impls ------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodeLeafBytesError<Raw: error::Error>(Raw);

impl<T, P: PtrBlob> Blob for Leaf<T, P>
where T: Blob,
{
    const SIZE: usize = <raw::Node<T, P> as Blob>::SIZE;

    type DecodeBytesError = DecodeLeafBytesError<<raw::Node<T, P> as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
            .write_field(&*self.raw)
            .done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();
        let raw = fields.trust_field().map_err(DecodeLeafBytesError)?;
        fields.assert_done();

        let this = unsafe { Self::from_raw(raw) };
        Ok(this.into())
    }
}

impl<T, P: Ptr> Load for Leaf<T, P>
where T: Load,
{
    type Blob = Leaf<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        let raw = raw::Node::load(blob.into_raw(), zone);

        unsafe {
            Self::from_raw(raw)
        }
    }
}

// ----- save impls ---------

/*
impl<Q: PtrClean, R: PtrBlob, T, P: Ptr> Save<Q, R> for Leaf<T, P>
where T: Save<Q, R>,
      Q: From<P::Clean>,
      Q::Zone: AsZone<P::Zone>,
{
    type SrcBlob = Leaf<T::SrcBlob, P::Blob>;
    type DstBlob = Leaf<T::DstBlob, R>;
    type SavePoll = LeafSavePoll<T, P::Clean, T::SavePoll, R>;

    fn init_save(&self) -> Self::SavePoll {
        LeafSavePoll {
            marker: PhantomData,
            digest: Digest::default(), //self.digest().cast(),
            state: match self.try_get_dirty() {
                Ok(dirty) => State::Dirty(dirty.init_save()),
                Err(p_clean) => State::Clean(p_clean.to_blob()),
            }
        }
    }

    fn init_save_from_blob(_blob: &Self::SrcBlob) -> Self::SavePoll {
        todo!()
    }
}

#[doc(hidden)]
pub struct LeafSavePoll<T, P: PtrClean, S, R> {
    marker: PhantomData<fn(T)>,
    digest: Digest,
    state: State<P, S, R>,
}

#[derive(Debug)]
enum State<P: PtrClean, S, R> {
    Clean(P::Blob),
    Dirty(S),
    Done(R),
}

/*
impl<T: SaveDirtyPoll, P: PtrConst> LeafSaveDirtyPoll<T, P> {
    pub(crate) fn encode_raw_node_blob(&self) -> raw::Node<T::SavedBlob, (), P::Blob> {
        match self.state {
            State::Done(ptr) => raw::Node::new(Some(self.digest), (), ptr),
            State::Dirty(_) => panic!(),
        }
    }
}
*/

impl<Q: PtrClean, R: PtrBlob, T, P: PtrClean> SavePoll<Q, R> for LeafSavePoll<T, P, T::SavePoll, R>
where T: Save<Q, R>,
      Q: From<P>,
      Q::Zone: AsZone<P::Zone>,
{
    type DstBlob = Leaf<T::DstBlob, R>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
    {
        loop {
            self.state = match &mut self.state {
                State::Clean(p_blob) => {
                    match saver.try_save_ptr::<P, T>(*p_blob, ())? {
                        Ok(r_ptr) => State::Done(r_ptr),
                        Err(target_poll) => State::Dirty(target_poll),
                    }
                },
                State::Dirty(target_poll) => {
                    target_poll.save_poll(saver)?;

                    let r_ptr = saver.save_blob_with((), |dst| {
                        target_poll.encode_blob_bytes(dst)
                    })?;

                    State::Done(r_ptr)
                },
                State::Done(_) => break Ok(()),
            }
        }
    }

    fn encode_blob(&self) -> Self::DstBlob {
        todo!()
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test_commit() {
        let n = 42u8;
        let leaf_n = Leaf::new_in(n, Heap);

        assert!(leaf_n.try_digest().is_none());
        assert_eq!(leaf_n.digest(), n.commit());
        assert_eq!(leaf_n.try_digest(), Some(n.commit().cast()));
        assert_eq!(leaf_n.commit(), n.commit());
    }

    #[test]
    fn test_digest_updated_on_write() {
        let n = 1u8;
        let mut leaf_n = Leaf::new_in(n, Heap);

        *(leaf_n.get_mut()) = 2;

        assert!(leaf_n.try_digest().is_none());
        assert_eq!(leaf_n.digest(), 2u8.commit());
        assert!(leaf_n.try_digest().is_some());

        leaf_n.get_mut();
        assert!(leaf_n.try_digest().is_none());
        assert_eq!(leaf_n.digest(), 2u8.commit());

        *(leaf_n.get_mut()) = 3;

        assert!(leaf_n.try_digest().is_none());
        assert_eq!(leaf_n.digest(), 3u8.commit());
        assert!(leaf_n.try_digest().is_some());
    }
}
*/
*/
