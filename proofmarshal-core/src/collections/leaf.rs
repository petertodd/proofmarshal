use std::borrow::{Borrow, BorrowMut};
use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::mem::{self, ManuallyDrop};
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

use crate::commit::{Commit, WriteVerbatim, Digest};

use super::raw;

/// Leaf node in a tree.
#[repr(transparent)]
pub struct Leaf<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    raw: ManuallyDrop<raw::Node<T, Z, P>>,
}

impl<T, Z, P: Ptr> Drop for Leaf<T, Z, P> {
    fn drop(&mut self) {
        unsafe {
            self.raw.ptr.dealloc::<T>(())
        }
    }
}

impl<T, Z: Zone> Leaf<T, Z> {
    pub fn new_in(value: T, mut zone: impl BorrowMut<Z>) -> Self
        where Z: Alloc,
    {
        Self::new_unchecked(None, zone.borrow_mut().alloc(value))
    }
}

impl<T, Z, P: Ptr> Leaf<T, Z, P> {
    pub fn new_unchecked(digest: Option<Digest>, bag: Bag<T, Z, P>) -> Self {
        let (ptr, (), zone) = bag.into_raw_parts();
        let raw = raw::Node::new(digest, zone, ptr);

        unsafe {
            Self::from_raw(raw)
        }
    }

    pub unsafe fn from_raw(raw: raw::Node<T, Z, P>) -> Self {
        Self {
            raw: ManuallyDrop::new(raw),
        }
    }

    pub unsafe fn from_raw_node_ref(raw: &raw::Node<T, Z, P>) -> &Self {
        &*(raw as *const _ as *const _)
    }

    pub unsafe fn from_raw_node_mut(raw: &mut raw::Node<T, Z, P>) -> &mut Self {
        &mut *(raw as *mut _ as *mut _)
    }

    pub fn into_raw(self) -> raw::Node<T, Z, P> {
        let this = ManuallyDrop::new(self);
        unsafe {
            ptr::read(&*this.raw)
        }
    }

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
}

impl<T, Z: Zone> Leaf<T, Z>
where T: Load
{
    pub fn get(&self) -> Ref<T>
        where Z: Get + AsZone<T::Zone>
    {
        unsafe {
            self.raw.get_unchecked::<T>(())
                    .trust()
        }
    }

    pub fn get_mut(&mut self) -> &mut T
        where Z: GetMut + AsZone<T::Zone>
    {
        unsafe {
            self.raw.get_unchecked_mut::<T>(())
                    .trust()
        }
    }

    pub fn take(self) -> T
        where Z: Get + AsZone<T::Zone>
    {
        let raw = self.into_raw();
        unsafe {
            raw.take_unchecked::<T>(())
               .trust()
        }
    }
}

impl<T, Z, P: Ptr> Leaf<T, Z, P> {
    pub fn try_get_dirty(&self) -> Result<&T, P::Clean> {
        unsafe {
            self.raw.try_get_dirty(())
        }
    }
}

impl<T, Z, P: Ptr> Commit for Leaf<T, Z, P>
where T: Commit
{
    const VERBATIM_LEN: usize = Digest::<!>::LEN;
    type Committed = T::Committed;

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.digest().as_bytes())
    }
}

// ---- hoard impls ------

#[derive(Debug, Error)]
#[error("FIXME")]
#[doc(hidden)]
pub struct DecodeLeafBytesError<Raw: error::Error>(Raw);

impl<T, Z, P: PtrBlob> Blob for Leaf<T, Z, P>
where T: Blob,
      Z: Blob,
{
    const SIZE: usize = <raw::Node<T, Z, P> as Blob>::SIZE;

    type DecodeBytesError = DecodeLeafBytesError<<raw::Node<T, Z, P> as Blob>::DecodeBytesError>;

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

impl<T, Z, P: Ptr> Load for Leaf<T, Z, P>
where T: Load,
      Z: Zone,
{
    type Blob = Leaf<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        let raw = raw::Node::load(blob.into_raw(), zone);

        unsafe {
            Self::from_raw(raw)
        }
    }
}

impl<T, Z, P: Ptr> fmt::Debug for Leaf<T, Z, P>
where T: fmt::Debug, Z: fmt::Debug, P: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Leaf")
            .field("digest", &self.raw.digest())
            .field("zone", &self.raw.zone)
            .field("ptr", &self.try_get_dirty().map_err(P::from_clean))
            .finish()
    }
}

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
