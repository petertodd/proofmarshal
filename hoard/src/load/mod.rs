use std::mem;

use crate::pointee::Pointee;

pub mod blob;
pub use self::blob::*;

pub trait Load : Pointee {
    type Error : std::error::Error;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

/*
pub trait Validate<'a, P> {
    type State;

    fn init_validate_state(&'a self) -> Self::State;
    fn poll<V: ValidatePtr<P>>(&'a self, state: &mut Self::State, validator: &mut V) -> Result<(), V::Error>;
}

pub trait ValidatePtr<P> {
    type Error;

    fn validate_ptr<T: ?Sized + Load>(&self, ptr: &P, metadata: T::Metadata) -> Result<Option<&T>, Self::Error>;
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
