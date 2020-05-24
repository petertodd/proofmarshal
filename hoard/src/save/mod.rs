use crate::pointee::Pointee;

pub mod blob;
pub use self::blob::*;

use crate::blob::*;

pub mod impls;

pub trait Encoded<R> {
    type Encoded : ValidateBlob;
}

pub trait Saved<R> : Pointee {
    type Saved : ?Sized + Pointee<Metadata=Self::Metadata> + BlobLen;
}

impl<R, T: Encoded<R>> Saved<R> for T {
    type Saved = T::Encoded;
}

pub trait Save<'a, Q, R> : Saved<R> {
    type State;

    fn init_save_state(&'a self) -> Self::State;

    fn save_poll<D>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>;

    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob;
}

pub trait Encode<'a, Q, R> : Encoded<R> {
    type State;

    fn init_encode_state(&'a self) -> Self::State;

    fn encode_poll<D>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>;

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob;
}

impl<'a, Q, R, T: Encode<'a, Q, R>> Save<'a, Q, R> for T {
    type State = T::State;

    fn init_save_state(&'a self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        self.encode_poll(state, dst)
    }

    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob
    {
        let dst = dst.alloc(<T::Encoded as ValidateBlob>::BLOB_LEN)?;
        self.encode_blob(state, dst)
    }
}

pub trait Dumper : Sized {
    type Source;
    type Target;

    type Error : std::error::Error;

    unsafe fn try_save_ptr<'a, T: ?Sized>(&mut self, ptr: &'a Self::Source, metadata: T::Metadata) -> Result<Self::Target, &'a T>
        where T: Pointee;

    fn save_ptr<'a, T: ?Sized>(self, value: &'a T, state: &T::State) -> Result<(Self, Self::Target), Self::Error>
        where T: Save<'a, Self::Source, Self::Target>;

}
