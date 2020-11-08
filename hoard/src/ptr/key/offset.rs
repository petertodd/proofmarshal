use std::marker::PhantomData;
use std::convert::TryFrom;
use std::cmp;

use thiserror::Error;

use crate::blob::{BlobDyn, Bytes, BytesUninit};
use crate::primitive::Primitive;
use crate::ptr::{Ptr, PtrClean, PtrBlob, AsZone, TryGet, Zone};
use crate::save::{SaveRef, SaveRefPoll, Saver};
use crate::load::LoadRef;
use crate::pointee::Pointee;

use super::{Key, Map};

/// Slice offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offset(u64);

impl Offset {
    #[inline]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Primitive for Offset {
    const BLOB_SIZE: usize = 8;
    type DecodeBytesError = !;

    #[inline]
    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_bytes(&self.0.to_le_bytes())
    }

    #[inline]
    fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let buf = TryFrom::try_from(&blob[..]).unwrap();
        Ok(Self::new(u64::from_le_bytes(buf)))
    }
}

impl From<!> for Offset {
    #[inline]
    fn from(never: !) -> Self { never }
}

impl PtrBlob for Offset {
}

impl From<Offset> for u64 {
    #[inline]
    fn from(offset: Offset) -> u64 {
        offset.0
    }
}

macro_rules! impl_cmp {
    ($( $lhs:ty => $rhs:ty; )+ ) => {$(
        impl cmp::PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, rhs: &$rhs) -> bool {
                let lhs = u64::from(*self);
                let rhs = u64::from(*rhs);
                lhs == rhs
            }
        }
    )+}
}

impl_cmp! {
    Offset => u64;
    u64 => Offset;
}


#[derive(Debug)]
pub struct DirtyOffsetSaver<'m, M: ?Sized> {
    map: &'m M,
    initial_offset: usize,
    dst: Vec<u8>,
}

impl<'m, M: ?Sized> DirtyOffsetSaver<'m, M>
where M: AsRef<[u8]>,
{
    pub fn new(map: &'m M) -> Self {
        Self {
            initial_offset: map.as_ref().len(),
            map,
            dst: vec![],
        }
    }
}

/*
impl<'m, M: ?Sized> DirtyOffsetSaver<'m, M>
where M: Map<Key = Offset>
{
    pub fn save<T: ?Sized>(mut self, value: &T) -> (Offset, Vec<u8>)
        where T: SaveRef<Key<'m, M>, Offset>
    {
        let mut poll = value.init_save_ref();

        poll.save_ref_poll(&mut self).into_ok();

        let metadata = poll.blob_metadata();
        let offset = self.save_blob_with(poll.blob_metadata(), |dst| {
            poll.encode_blob_dyn_bytes(dst)
        }).into_ok();

        (offset, self.dst)
    }
}
*/

#[derive(Debug)]
pub struct OffsetSaver<'m, M: ?Sized> {
    map: &'m M,
    dst: Vec<u8>,
}

impl<'m, M: ?Sized> OffsetSaver<'m, M>
where M: Map<Key = Offset> + AsRef<[u8]>
{
    pub fn new(map: &'m M) -> Self {
        Self {
            map,
            dst: vec![],
        }
    }

    pub fn try_save<T: ?Sized>(mut self, value: &T) -> Result<(Offset, Vec<u8>), Box<dyn std::error::Error>>
        where T: SaveRef<Offset>,
              Key<'m, M>: From<<T::Ptr as Ptr>::Clean>,
              &'m M: AsZone<<T::Ptr as Ptr>::Zone>,
    {

        let wrapper: &mut Wrapper<Self, <T::Ptr as Ptr>::Clean> = Wrapper::new(&mut self);

        let mut poll = value.init_save_ref();
        let offset = wrapper.poll_ref::<T::SaveRefPoll>(&mut poll)?;

        Ok((offset, self.dst))
    }
}


trait BlobSaver {
    type MapError : std::error::Error + 'static + Send;
    type SaveError : std::error::Error + 'static + Send;

    type Key : PtrClean;

    fn zone(&self) -> &<Self::Key as PtrClean>::Zone;

    fn get_blob_with<T: ?Sized, F, R>(
        &self,
        key: Self::Key,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Result<Offset, R>, Self::MapError>
        where T: BlobDyn,
              F: FnOnce(Bytes<'_, T>) -> R;

    fn save_blob_with<T: ?Sized, F>(
        &mut self,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Offset, Self::SaveError>
        where T: BlobDyn,
              F: for<'a> FnOnce(BytesUninit<'a, T>) -> Bytes<'a, T>;
}

impl<'m, M: ?Sized> BlobSaver for OffsetSaver<'m, M>
where M: Map
{
    type MapError = M::Error;
    type SaveError = !;

    type Key = Key<'m, M>;

    fn zone(&self) -> &<Self::Key as PtrClean>::Zone {
        &self.map
    }

    fn get_blob_with<T: ?Sized, F, R>(
        &self,
        key: Self::Key,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Result<Offset, R>, Self::MapError>
        where T: BlobDyn,
              F: FnOnce(Bytes<'_, T>) -> R
    {
        let r = self.map.get_blob_with(key.key, metadata, f)?;
        Ok(Err(r))
    }

    fn save_blob_with<T: ?Sized, F>(
        &mut self,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Offset, Self::SaveError>
        where T: BlobDyn,
              F: for<'a> FnOnce(BytesUninit<'a, T>) -> Bytes<'a, T>
    {
        let size = T::try_size(metadata).expect("valid metadata");

        let old_len = self.dst.len();
        self.dst.resize(old_len + size, 0);

        let dst = &mut self.dst[old_len ..];
        let dst = BytesUninit::<T>::from_bytes(dst, metadata).expect("valid metadata");

        f(dst);
        Ok(Offset::new(old_len as u64))
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct Wrapper<S, P> {
    marker: PhantomData<fn() -> P>,
    inner: S,
}

impl<S, P> Wrapper<S, P> {
    fn new(inner: &mut S) -> &mut Self {
        // SAFETY: #[repr(transparent)]
        unsafe { &mut *(inner as *mut S as *mut Self) }
    }
}

impl<P: PtrClean, S: BlobSaver> BlobSaver for Wrapper<S, P>
where S::Key: From<P>,
      <S::Key as PtrClean>::Zone: AsZone<P::Zone>
{
    type MapError = S::MapError;
    type SaveError = S::SaveError;

    type Key = P;

    fn zone(&self) -> &<Self::Key as PtrClean>::Zone {
        self.inner.zone().as_zone()
    }

    fn get_blob_with<T: ?Sized, F, R>(
        &self,
        key: Self::Key,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Result<Offset, R>, Self::MapError>
        where T: BlobDyn,
              F: FnOnce(Bytes<'_, T>) -> R
    {
        self.inner.get_blob_with(key.into(), metadata, f)
    }

    fn save_blob_with<T: ?Sized, F>(
        &mut self,
        metadata: T::Metadata,
        f: F,
    ) -> Result<Offset, Self::SaveError>
        where T: BlobDyn,
              F: for<'a> FnOnce(BytesUninit<'a, T>) -> Bytes<'a, T>
    {
        self.inner.save_blob_with(metadata, f)
    }
}

impl<S: BlobSaver, P: PtrClean> Saver for Wrapper<S, P>
where S::Key: From<P>,
      <S::Key as PtrClean>::Zone: AsZone<P::Zone>
{
    type Error = Box<dyn std::error::Error>;
    type SrcPtr = P;
    type DstPtr = Offset;

    fn save_ptr<T: ?Sized>(
        &mut self,
        key: Self::SrcPtr,
        metadata: T::Metadata,
    ) -> Result<Result<Offset, T::SaveRefPoll>, Self::Error>
    where
        T: SaveRef<Offset>,
        <Self::SrcPtr as Ptr>::Zone: AsZone<T::Zone>,
    {
        let r = self.get_blob_with(key, metadata, |bytes| {
            T::init_save_ref_from_bytes(bytes, self.zone().as_zone())
        })?;

        match r {
            Ok(offset) => Ok(Ok(offset)),
            Err(Ok(poll)) => Ok(Err(poll)),
            Err(Err(decode_err)) => Err(decode_err.into())
        }
    }

    fn poll<T: ?Sized>(&mut self, value: &mut T) -> Result<(), Self::Error>
        where T: SaveRefPoll<DstPtr = Self::DstPtr>,
              Self::SrcPtr: From<T::SrcPtr>,
              <Self::SrcPtr as Ptr>::Zone: AsZone<<T::SrcPtr as Ptr>::Zone>,
    {
        let coerced: &mut Wrapper<Self, T::SrcPtr> = Wrapper::new(self);
        value.save_ref_poll(coerced)
    }

    fn poll_ref<T: ?Sized>(&mut self, value: &mut T) -> Result<Self::DstPtr, Self::Error>
        where T: SaveRefPoll<DstPtr = Self::DstPtr>,
              Self::SrcPtr: From<T::SrcPtr>,
              <Self::SrcPtr as Ptr>::Zone: AsZone<<T::SrcPtr as Ptr>::Zone>,
    {
        let coerced: &mut Wrapper<Self, T::SrcPtr> = Wrapper::new(self);
        value.save_ref_poll(coerced)?;

        let offset = self.save_blob_with(value.blob_metadata(), |dst| {
            value.encode_blob_dyn_bytes(dst)
        })?;
        Ok(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ptr::{
        Ptr,
        Heap,
        key::KeyMut,
    };
    use crate::bag::Bag;

    #[test]
    fn offset_saver_u8() {
        let map: &[u8] = &[];
        let saver = OffsetSaver::new(map);

        let (offset, buf) = saver.try_save(&42u8).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(buf, &[42]);
    }

    #[test]
    fn offset_saver_bag() {
        let map: &[u8] = &[];

        let bag = KeyMut::<[u8]>::alloc(42u8);
        let saver = OffsetSaver::new(map);
        let (offset, buf) = saver.try_save(&bag).unwrap();
        assert_eq!(offset, 1);
        assert_eq!(buf, &[
            42,
            0,0,0,0,0,0,0,0
        ]);

        let bag = KeyMut::<[u8]>::alloc(bag);
        let saver = OffsetSaver::new(map);
        let (offset, buf) = saver.try_save(&bag).unwrap();
        assert_eq!(offset, 9);
        assert_eq!(buf, &[
            42,
            0,0,0,0,0,0,0,0,
            1,0,0,0,0,0,0,0,
        ]);

        let bag = Heap::alloc(Heap::alloc(Heap::alloc(32u8)));
        let saver = OffsetSaver::new(map);
        let (offset, buf) = saver.try_save(&bag).unwrap();
        assert_eq!(offset, 17);
        assert_eq!(buf, &[
            32,
            0,0,0,0,0,0,0,0,
            1,0,0,0,0,0,0,0,
            9,0,0,0,0,0,0,0,
        ]);
    }
}
