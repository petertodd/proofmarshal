use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::error;

use thiserror::Error;

use hoard::blob::{Blob, Bytes, BytesUninit};
use hoard::load::{Load, LoadRef, MaybeValid};
use hoard::owned::Ref;
use hoard::zone::{Get, GetMut, TryGet, TryGetMut, Zone, AsZone, Ptr, PtrConst};
use hoard::pointee::Pointee;

use crate::commit::Digest;

/// A raw, untyped, tree node.
#[derive(Debug)]
pub struct Node<T, Z, P> {
    marker: PhantomData<T>,
    digest: Cell<Option<Digest>>,
    pub zone: Z,
    pub ptr: P,
}

/// A pair of left and right `Node`\'s.
#[derive(Debug)]
pub struct Pair<T, Z, P> {
    pub left: Node<T, Z, P>,
    pub right: Node<T, Z, P>,
}

impl<T, Z, P> Pair<T, Z, P> {
    pub fn split_mut(&mut self) -> (&mut Node<T, Z, P>, &mut Node<T, Z, P>) {
        (&mut self.left,
         &mut self.right)
    }
}

impl<T, Z, P> Node<T, Z, P> {
    pub fn new(digest: Option<Digest>, zone: Z, ptr: P) -> Self {
        Self {
            marker: PhantomData,
            digest: digest.into(),
            zone,
            ptr,
        }
    }

    pub fn into_raw_parts(self) -> (Option<Digest>, Z, P) {
        (self.digest.into_inner(),
         self.zone,
         self.ptr)
    }

    pub fn digest(&self) -> Option<Digest> {
        self.digest.get()
    }

    pub fn set_digest(&self, digest: Digest) {
        self.digest.set(Some(digest));
    }
}

impl<T, Z, P: Ptr> Node<T, Z, P> {
    pub unsafe fn get_unchecked<U: ?Sized + LoadRef>(&self, metadata: U::Metadata) -> MaybeValid<Ref<U>>
        where Z: Get<P> + AsZone<U::Zone>
    {
        self.zone.get_unchecked::<U>(&self.ptr, metadata)
    }

    pub unsafe fn get_unchecked_mut<U: ?Sized + LoadRef>(&mut self, metadata: U::Metadata) -> MaybeValid<&mut U>
        where Z: GetMut<P> + AsZone<U::Zone>
    {
        let r = self.zone.get_unchecked_mut::<U>(&mut self.ptr, metadata);
        self.digest.take();
        r
    }

    pub unsafe fn take_unchecked<U: ?Sized + LoadRef>(self, metadata: U::Metadata) -> MaybeValid<U::Owned>
        where Z: Get<P> + AsZone<U::Zone>
    {
        self.zone.take_unchecked::<U>(self.ptr, metadata)
    }

    pub unsafe fn try_get_dirty<U: ?Sized + Pointee>(&self, metadata: U::Metadata) -> Result<&U, P::Clean> {
        self.ptr.try_get_dirty(metadata)
    }
}

#[doc(hidden)]
#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeNodeBytesError<Z: error::Error, P: error::Error> {
    Zone(Z),
    Ptr(P),
}

impl<T, Z, P> Blob for Node<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + Z::SIZE + P::SIZE;

    type DecodeBytesError = DecodeNodeBytesError<Z::DecodeBytesError, P::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.digest.get().expect("digest missing"))
           .write_field(&self.zone)
           .write_field(&self.ptr)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let digest = fields.trust_field().into_ok();
        let zone = fields.trust_field().map_err(DecodeNodeBytesError::Zone)?;
        let ptr = fields.trust_field().map_err(DecodeNodeBytesError::Ptr)?;
        fields.assert_done();
        Ok(Self::new(Some(digest), zone, ptr).into())
    }
}

impl<T, Z, P> Load for Node<T, Z, P>
where T: Load,
      Z: Zone,
      P: Ptr,
{
    type Blob = Node<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        let ptr = P::from_clean(P::Clean::from_blob(blob.ptr));

        let digest = blob.digest().unwrap();
        Self::new(Some(digest), zone.clone(), ptr)
    }
}

#[doc(hidden)]
#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodePairBytesError<E: error::Error> {
    Left(E),
    Right(E),
}

impl<T, Z, P> Blob for Pair<T, Z, P>
where T: 'static,
      Z: Blob,
      P: Blob,
{
    const SIZE: usize = <Node<T, Z, P> as Blob>::SIZE * 2;

    type DecodeBytesError = DecodePairBytesError<<Node<T, Z, P> as Blob>::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.left)
           .write_field(&self.right)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let left = fields.trust_field().map_err(DecodePairBytesError::Left)?;
        let right = fields.trust_field().map_err(DecodePairBytesError::Left)?;
        fields.assert_done();
        Ok(Self {
            left,
            right,
        }.into())
    }
}

impl<T, Z, P> Load for Pair<T, Z, P>
where T: Load,
      Z: Zone,
      P: Ptr,
{
    type Blob = Pair<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Z) -> Self {
        Self {
            left: Load::load(blob.left, zone),
            right: Load::load(blob.right, zone),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest() {
        let node: Node<u8, (), ()> = Node::new(None, (), ());
        assert!(node.digest().is_none());

        let digest = Digest::default();
        let node: Node<u8, (), ()> = Node::new(Some(digest), (), ());
        assert_eq!(node.digest().unwrap(), digest);
    }

    #[test]
    fn test_blob_encode() {
        let node: Node<u8, u16, u32> = Node::new(Some(Digest::default()), 16, 32);
        assert_eq!(node.to_blob_bytes(),
                   vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        16, 0,
                        32, 0, 0, 0]);
    }
}
