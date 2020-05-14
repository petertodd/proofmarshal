use thiserror::Error;

use crate::pointee::Pointee;
use crate::load::*;

#[repr(C)]
pub struct Fat<T: ?Sized + Pointee, P> {
    pub raw: P,
    pub metadata: T::Metadata,
}

/*
impl<Q, T: ?Sized + Pointee, P: Load> Load<Q> for Fat<T,P> {
    type Error = LoadError<P::Error, <T::Metadata as Load>::Error>;
    type State = ();

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }

    fn init_validate_state(&self) -> Self::State {
    }

    fn poll<V: ValidatePtr<Q>>(_: &mut Self::State, _: &V) -> Result<(), V::Error> {
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum LoadError<P: std::fmt::Debug, M: std::fmt::Debug> {
    #[error("invalid pointer: {0:?}")]
    Pointer(P),

    #[error("invalid metadata: {0:?}")]
    Metadata(M),
}
*/
