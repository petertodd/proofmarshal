//! Saving values.
use crate::pointee::Pointee;

pub mod blob;
pub use self::blob::*;

pub mod impls;

pub trait Encode<Q, R> {
    type EncodePoll : SavePoll<Q, R> + EncodeBlob;

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll;
}

pub trait EncodeBlob {
    const BLOB_LEN: usize;
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error>;
}

pub trait SavePoll<Q, R> {
    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>;
}

pub trait Save<Q, R> {
    type SavePoll : SavePoll<Q, R> + SaveBlob;
    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll;
}

pub trait SaveBlob {
    fn save_blob<W: AllocBlob>(&self, dst: W) -> Result<W::Done, W::Error>;
}

impl<Q, R, T: Encode<Q, R>> Save<Q, R> for T {
    type SavePoll = T::EncodePoll;

    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll {
        self.init_encode(dst)
    }
}

impl<T: EncodeBlob> SaveBlob for T {
    fn save_blob<W: AllocBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc_blob(T::BLOB_LEN)?;
        self.encode_blob(dst)
    }
}

pub trait SavePtr : Sized {
    type Source;
    type Target;
    type Error;

    unsafe fn check_dirty<'a, T: ?Sized>(&self, ptr: &'a Self::Source, metadata: T::Metadata) -> Result<Self::Target, &'a T>
        where T: Pointee;

    fn try_save_ptr(self, saver: &impl SaveBlob) -> Result<(Self, Self::Target), Self::Error>;
}

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for &'_ mut T {
    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>
    {
        (**self).save_poll(dst)
    }
}

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for Box<T> {
    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>
    {
        (**self).save_poll(dst)
    }
}
