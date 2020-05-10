pub mod writeblob;
pub use self::writeblob::*;

pub trait Save<P> {
    type State;

    fn init_save_state(&self) -> Self::State;

    unsafe fn poll<D: SavePtr<P>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error>;
    unsafe fn encode<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;
}

pub trait SavePtr<P> : Sized {
    type Error : std::error::Error;

    type WriteBlob : WriteBlob<Ok=Self::WriteBlobOk, Error=Self::WriteBlobError>;
    type WriteBlobOk;
    type WriteBlobError;

    /// Saves a blob.
    fn save_blob(self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, P), Self::Error>;
}

impl SavePtr<!> for ! {
    type Error = !;

    type WriteBlob = !;
    type WriteBlobOk = !;
    type WriteBlobError = !;

    fn save_blob(self,
        _size: usize,
        _f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, !), Self::Error>
    {
        match self {}
    }
}
