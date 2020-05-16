use std::mem;

use crate::pointee::Pointee;

pub mod blob;
pub use self::blob::*;

pub trait Saved<R> : Pointee {
    type Saved : ?Sized + Pointee<Metadata=Self::Metadata>;
}

pub trait Save<Q, R> : Saved<R> {
    type Thunk : SavePoll<Q, R, Target=Self::Saved>;

    fn save_children<D>(&self, dst: &mut D) -> Self::Thunk
        where D: SavePtr<Source=Q, Target=R>;
}

pub trait SavePoll<Q, R> {
    type Target : ?Sized;

    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error>
        where Self::Target: Sized;

    fn save_blob<W: SaveBlob>(&self, dst: W) -> Result<W::Done, W::Error>;
}

pub trait SavePtr : Sized {
    type Source;
    type Target;

    type Error : std::error::Error;

    /*
    fn save<'a, T: ?Sized>(self, value: &'a T, state: &T::State) -> Result<(Self, R), Self::Error>
        where T: Save<'a, Q, R>;

    unsafe fn try_save_ptr<'a, T: ?Sized>(&mut self, ptr: &'a Q, metadata: T::Metadata) -> Result<R, &'a T>
        where T: Pointee;
    */
}
