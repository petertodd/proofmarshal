use std::mem;

pub mod blob;
pub use self::blob::*;

pub trait Persist {
    type Error : std::error::Error;
    fn deref_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
