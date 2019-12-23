use core::mem::{self, MaybeUninit};

use crate::pointee::Pointee;
use crate::zone::{Zone, PersistZone, ValidPtr, FatPtr};

pub mod impls;

mod writeblob;
pub use self::writeblob::WriteBlob;

pub trait Encoded<Z> {
    type Encoded;
}

pub trait Encode<'a, Z: Zone> : 'a + Encoded<Z> {
    type State;

    fn save_children(&'a self) -> Self::State;

    fn poll<D>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error>
        where D: Dumper<Z>;

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;

    fn zone_save_ptr<T, D>(ptr: &'a ValidPtr<T, Self>, dumper: &D) -> Result<D::PersistPtr, &'a T>
        where Self: Zone,
              T: ?Sized + Pointee,
              D: Dumper<Z>
    {
        todo!()
    }
}

pub trait Saved<Z> : Pointee {
    type Saved : ?Sized + Pointee;
}

impl<Z, T: Encoded<Z>> Saved<Z> for T {
    type Saved = T::Encoded;
}

pub trait Save<'a, Z: Zone> : Saved<Z> {
    type State;

    fn save_children(&'a self) -> Self::State;
    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error>;

    fn save_blob<D: Dumper<Z>>(&self, state: &Self::State, dumper: D) -> Result<(D, D::PersistPtr), D::Error>;
}

impl<'a, Z: Zone, T: Encode<'a, Z>> Save<'a, Z> for T {
    type State = T::State;

    fn save_children(&'a self) -> Self::State {
        self.save_children()
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        self.poll(state, dumper)
    }

    fn save_blob<D: Dumper<Z>>(&self, state: &Self::State, dumper: D) -> Result<(D, D::PersistPtr), D::Error> {
        /*
        dumper.save_blob(mem::size_of::<T::Encoded>(), |dst| {
            self.encode_blob(state, dst)
        })
        */ todo!()
    }
}

/*
pub trait Primitive : Encode<!> {
    fn encode_to_vec(&self) -> Vec<u8> {
        /*
        match self.encode_blob(&Default::default(), vec![]) {
            Ok(vec) => vec,
            Err(never) => match never {}
        }
        */ todo!()
    }
}
*/

/*
impl<T: Encode<!>> Primitive for T
where <Self as SavePoll<'static, !>>::State: Default
{}
*/

pub trait Dumper<Y: Zone> : Sized {
    type Error;
    type PersistPtr;

    type WriteBlob : WriteBlob<Ok=Self::WriteBlobOk, Error=Self::WriteBlobError>;
    type WriteBlobOk;
    type WriteBlobError;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn save_ptr<'a, T: ?Sized + Pointee>(&self, ptr: &'a ValidPtr<T, Y>) -> Result<Self::PersistPtr, &'a T>;

    /// Saves a blob.
    fn save_blob(self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, Self::PersistPtr), Self::Error>;

    fn coerce_ptr(&self, ptr: Self::PersistPtr) -> <Y::Persist as Zone>::Ptr;
}

/*
impl Dumper<!> for Vec<u8> {
    type Error = !;
    type PersistPtr = ();

    type WriteBlob : WriteBlob<Ok=Self::WriteBlobOk, Error=Self::WriteBlobError>;
    type WriteBlobOk;
    type WriteBlobError;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn save_ptr<'a, T: ?Sized + Pointee>(&self, ptr: &'a ValidPtr<T, Z>) -> Result<FatPtr<T, Z::Persist>, &'a T>;

    /// Saves a blob.
    fn save_blob(self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, Self::PersistPtr), Self::Error>;
}
*/
