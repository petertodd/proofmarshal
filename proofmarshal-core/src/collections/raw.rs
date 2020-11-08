use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::error;

use thiserror::Error;

use hoard::blob::{Blob, Bytes, BytesUninit};
use hoard::load::{Load, LoadRef, MaybeValid};
use hoard::owned::Ref;
use hoard::ptr::{Get, GetMut, TryGet, TryGetMut, Zone, AsZone, Ptr, PtrClean};
use hoard::pointee::Pointee;

use crate::commit::Digest;

/// A raw, untyped, tree node.
#[derive(Debug)]
pub struct Node<T, P> {
    marker: PhantomData<T>,
    digest: Cell<Option<Digest>>,
    pub ptr: P,
}

/// A pair of left and right `Node`\'s.
#[derive(Debug)]
pub struct Pair<T, P> {
    pub left: Node<T, P>,
    pub right: Node<T, P>,
}

impl<T, P> Pair<T, P> {
    pub fn split_mut(&mut self) -> (&mut Node<T, P>, &mut Node<T, P>) {
        (&mut self.left,
         &mut self.right)
    }
}

impl<T, P> Node<T, P> {
    pub fn new(digest: Option<Digest>, ptr: P) -> Self {
        Self {
            marker: PhantomData,
            digest: digest.into(),
            ptr,
        }
    }

    pub fn into_raw_parts(self) -> (Option<Digest>, P) {
        (self.digest.into_inner(),
         self.ptr)
    }

    pub fn digest(&self) -> Option<Digest> {
        self.digest.get()
    }

    pub fn set_digest(&self, digest: Digest) {
        self.digest.set(Some(digest));
    }
}

impl<T, P: Ptr> Node<T, P> {
    pub unsafe fn get<U: ?Sized>(&self, metadata: U::Metadata) -> MaybeValid<Ref<U>>
        where U: LoadRef,
              P::Zone: AsZone<U::Zone>,
              P: Get
    {
        self.ptr.get::<U>(metadata)
    }

    pub unsafe fn get_mut<U: ?Sized + LoadRef>(&mut self, metadata: U::Metadata) -> MaybeValid<&mut U>
        where U: LoadRef,
              P::Zone: AsZone<U::Zone>,
              P: GetMut
    {
        let r = self.ptr.get_mut::<U>(metadata);
        self.digest.take();
        r
    }

    pub unsafe fn take<U: ?Sized + LoadRef>(self, metadata: U::Metadata) -> MaybeValid<U::Owned>
        where U: LoadRef,
              P::Zone: AsZone<U::Zone>,
              P: Get
    {
        self.ptr.take::<U>(metadata)
    }

    pub unsafe fn try_get_dirty<U: ?Sized + Pointee>(&self, metadata: U::Metadata) -> Result<MaybeValid<&U>, P::Clean> {
        self.ptr.try_get_dirty(metadata)
    }
}

#[doc(hidden)]
#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodeNodeBytesError<P: error::Error> {
    Ptr(P),
}

impl<T, P> Blob for Node<T, P>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <Digest as Blob>::SIZE + P::SIZE;

    type DecodeBytesError = DecodeNodeBytesError<P::DecodeBytesError>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.digest.get().expect("digest missing"))
           .write_field(&self.ptr)
           .done()
    }

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let digest = fields.trust_field().into_ok();
        let ptr = fields.trust_field().map_err(DecodeNodeBytesError::Ptr)?;
        fields.assert_done();
        Ok(Self::new(Some(digest), ptr).into())
    }
}

impl<T, P> Load for Node<T, P>
where T: Load,
      P: Ptr,
{
    type Blob = Node<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &P::Zone) -> Self {
        let ptr = P::from_clean(P::Clean::from_blob(blob.ptr, zone));

        let digest = blob.digest().unwrap();
        Self::new(Some(digest), ptr)
    }
}

#[doc(hidden)]
#[derive(Debug, Error)]
#[error("FIXME")]
pub enum DecodePairBytesError<E: error::Error> {
    Left(E),
    Right(E),
}

impl<T, P> Blob for Pair<T, P>
where T: 'static,
      P: Blob,
{
    const SIZE: usize = <Node<T, P> as Blob>::SIZE * 2;

    type DecodeBytesError = DecodePairBytesError<<Node<T, P> as Blob>::DecodeBytesError>;

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

impl<T, P> Load for Pair<T, P>
where T: Load,
      P: Ptr,
{
    type Blob = Pair<T::Blob, P::Blob>;
    type Ptr = P;
    type Zone = P::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
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
        let node: Node<u8, ()> = Node::new(None, ());
        assert!(node.digest().is_none());

        let digest = Digest::default();
        let node: Node<u8, ()> = Node::new(Some(digest), ());
        assert_eq!(node.digest().unwrap(), digest);
    }

    #[test]
    fn test_blob_encode() {
        let node: Node<u8, u32> = Node::new(Some(Digest::default()), 32);
        assert_eq!(node.to_blob_bytes(),
                   vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        32, 0, 0, 0]);
    }
}
