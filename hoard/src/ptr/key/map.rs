use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::ptr::NonNull;

use thiserror::Error;

use crate::blob::{BlobDyn, Bytes};

use super::*;

pub trait Map {
    type Id : 'static + Send + Copy + fmt::Debug + PartialEq + Eq;
    type Error : error::Error + 'static + Send;
    type Key : PtrBlob;

    fn id(&self) -> Self::Id;

    fn get_blob_with<T: ?Sized, F, R>(&self, key: Self::Key, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where F: FnOnce(Bytes<T>) -> R,
              T: BlobDyn;
}

impl<M: ?Sized + Map> Map for &'_ M {
    type Id = M::Id;
    type Error = M::Error;
    type Key = M::Key;

    fn id(&self) -> Self::Id {
        (**self).id()
    }

    fn get_blob_with<T: ?Sized, F, R>(&self, key: Self::Key, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where F: FnOnce(Bytes<T>) -> R,
              T: BlobDyn
    {
        (**self).get_blob_with(key, metadata, f)
    }
}

impl<'a, M: ?Sized + Map> Zone for &'a M {
    type Id = M::Id;
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("FIXME")]
pub struct SliceError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliceId(NonNull<[u8]>);
unsafe impl Send for SliceId {}

impl Map for [u8] {
    type Id = SliceId;
    type Error = SliceError;
    type Key = Offset;

    fn id(&self) -> Self::Id {
        SliceId(self.into())
    }

    fn get_blob_with<T: ?Sized, F, R>(&self, offset: Offset, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where F: FnOnce(Bytes<T>) -> R,
              T: BlobDyn
    {
        let len = T::try_size(metadata).expect("valid metadata");

        let start: usize = usize::try_from(offset.get()).ok().ok_or(SliceError)?;
        let end = start.checked_add(len).ok_or(SliceError)?;
        let buf: &[u8] = self.get(start .. end).ok_or(SliceError)?;

        let bytes = unsafe { Bytes::new_unchecked(buf.as_ptr(), metadata) };

        Ok(f(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_map() {
        let buf = &[0x12u8, 0x34u8, 0x56u8];

        buf.get_blob_with::<u16, _, _>(Offset::new(0), (), |src| {
            assert_eq!(&*src, &[0x12, 0x34]);
        }).unwrap();

        buf.get_blob_with::<u16, _, _>(Offset::new(1), (), |src| {
            assert_eq!(&*src, &[0x34, 0x56]);
        }).unwrap();

        assert_eq!(buf.get_blob_with::<u16, _, _>(Offset::new(2), (), |_| ()).unwrap_err(),
                   SliceError);
    }
}
