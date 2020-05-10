use thiserror::Error;

use crate::pointee::Pointee;
use crate::load::*;

#[repr(C)]
pub struct Fat<T: ?Sized + Pointee, P> {
    pub raw: P,
    pub metadata: T::Metadata,
}

impl<T: ?Sized + Pointee, P: Load> Load for Fat<T,P> {
    type Error = LoadError<P::Error, <T::Metadata as Load>::Error>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

#[derive(Error, Debug)]
pub enum LoadError<P: std::fmt::Debug, M: std::fmt::Debug> {
    #[error("invalid pointer: {0:?}")]
    Pointer(P),

    #[error("invalid metadata: {0:?}")]
    Metadata(M),
}
