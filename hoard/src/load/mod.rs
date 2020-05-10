use std::mem;

use crate::pointee::Pointee;

pub mod blob;
pub use self::blob::*;

pub trait Load : Pointee {
    type Error : std::error::Error;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
