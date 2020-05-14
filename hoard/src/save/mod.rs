use std::mem;

use crate::pointee::Pointee;
use crate::ptr::{Ptr, Bag, AsPtr};

pub mod blob;
pub use self::blob::*;

pub trait Saved<R> : Pointee {
    type Saved : ?Sized + Pointee<Metadata=Self::Metadata>;
}

pub trait Save<'a, Q, R> : Saved<R> {
    type State;

    fn init_save_state(&'a self) -> Self::State;

    fn save_poll<D: SavePtr<Q, R>>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error>;
    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>;

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where Self::Saved: Sized;
}

/*
impl<'a, Q, R, T: Encode<'a, Q, R>> Save<'a, Q, R> for T {
    type State = T::State;

    fn init_save_state(&'a self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D: SavePtr<Q, R>>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error> {
        self.encode_poll(state, dst)
    }

    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<T::Encoded>())?;
        self.encode_blob(state, dst)
    }
}
*/

pub trait SavePtr<Q, R> : Sized {
    type Error : std::error::Error;

    fn save<'a, T: ?Sized>(self, value: &'a T, state: &T::State) -> Result<(Self, R), Self::Error>
        where T: Save<'a, Q, R>;

    unsafe fn try_save_ptr<'a, T: ?Sized>(&mut self, ptr: &'a Q, metadata: T::Metadata) -> Result<R, &'a T>
        where T: Pointee;
}
