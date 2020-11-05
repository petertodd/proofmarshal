use std::convert::TryFrom;
use std::fmt;

use thiserror::Error;

use crate::blob::{Bytes, BlobDyn};

use super::Offset;

pub trait Mapping {
    type Error : 'static + std::error::Error;
    type Identifier : 'static + fmt::Debug;

    fn default_ref<'a>() -> &'a Self;

    fn deref_bytes<'a, T: ?Sized>(&'a self, offset: Offset, metadata: T::Metadata) -> Result<Bytes<'a, T>, Self::Error>
        where T: BlobDyn;

    fn identifier(&self) -> Self::Identifier;
}

#[derive(Error, Debug)]
#[non_exhaustive]
#[error("FIXME")]
pub struct SliceMappingError;

impl Mapping for [u8] {
    type Error = SliceMappingError;
    type Identifier = *const [u8];

    fn default_ref<'a>() -> &'a Self {
        &[]
    }

    fn deref_bytes<'a, T: ?Sized>(&'a self, offset: Offset, metadata: T::Metadata) -> Result<Bytes<'a, T>, Self::Error>
        where T: BlobDyn
    {
        let size = T::try_size(metadata).expect("valid metadata");
        let start = usize::try_from(offset.get())
                          .or(Err(SliceMappingError))?;
        let end = start.checked_add(size)
                       .ok_or(SliceMappingError)?;

        let slice = self.get(start .. end)
                        .ok_or(SliceMappingError)?;

        unsafe {
            Ok(Bytes::new_unchecked(slice.as_ptr(), metadata))
        }
    }

    fn identifier(&self) -> Self::Identifier {
        self
    }
}
