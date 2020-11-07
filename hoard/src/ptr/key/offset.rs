use std::marker::PhantomData;
use std::convert::TryFrom;
use std::cmp;

use thiserror::Error;

use crate::blob::{BlobDyn, Bytes, BytesUninit};
use crate::primitive::Primitive;
use crate::ptr::{PtrClean, PtrBlob, AsZone};
use crate::save::{SaveRef, SaveRefPoll, Saver};

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

impl<'m, M: ?Sized> Saver for DirtyOffsetSaver<'m, M>
where M: Map<Key = Offset>
{
    type Error = !;
    type SrcPtr = Key<'m, M>;
    type DstPtr = Offset;

    fn try_save_ptr<P, T: ?Sized>(
        &mut self,
        ptr: P::Blob,
        metadata: T::Metadata,
    ) -> Result<Result<Self::DstPtr, T::SavePoll>, Self::Error>
    where T: SaveRef<Self::SrcPtr, Self::DstPtr>,
          P: PtrClean + Into<Self::SrcPtr>,
          <Self::SrcPtr as PtrClean>::Zone: AsZone<P::Zone>
    {
        let zone: &P::Zone = (&self.map).as_zone();
        let p_ptr = P::from_blob(ptr, zone);
        let key: Key<M> = p_ptr.into();

        Ok(Ok(key.to_blob()))
    }

    fn save_blob_with<T: ?Sized, F>(&mut self, metadata: T::Metadata, f: F) -> Result<Self::DstPtr, Self::Error>
        where T: BlobDyn,
              F: for<'a> FnOnce(BytesUninit<'a, T>) -> Bytes<'a, T>
    {
        let size = T::try_size(metadata).expect("valid metadata");

        let old_len = self.dst.len();
        self.dst.resize(old_len + size, 0);

        let dst = &mut self.dst[old_len ..];
        let dst = BytesUninit::<T>::from_bytes(dst, metadata).expect("valid metadata");

        f(dst);
        Ok(Offset::new((self.initial_offset + old_len) as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ptr::{
        Ptr,
        key::KeyMut,
    };
    use crate::bag::Bag;

    #[test]
    fn saver() {
        let map: &[u8] = &[];
        let saver = DirtyOffsetSaver::new(map);

        let (offset, buf) = saver.save(&42u8);
        assert_eq!(offset, 0);
        assert_eq!(buf, &[42]);
    }

    #[test]
    fn saver_bag() {
        let map: &[u8] = &[];

        let bag = KeyMut::<[u8]>::alloc(42u8);

        let saver = DirtyOffsetSaver::new(map);
        let (offset, buf) = saver.save(&bag);
        assert_eq!(offset, 1);
        assert_eq!(buf, &[
            42,
            0,0,0,0,0,0,0,0
        ]);

        let bag = KeyMut::<[u8]>::alloc(bag);

        let saver = DirtyOffsetSaver::new(map);
        let (offset, buf) = saver.save(&bag);
        assert_eq!(offset, 9);
        assert_eq!(buf, &[
            42,
            0,0,0,0,0,0,0,0,
            1,0,0,0,0,0,0,0,
        ]);
    }
}
