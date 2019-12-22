//! Serialization of types that do contain pointers.

use core::any::Any;
use core::convert::TryFrom;
use core::fmt;
use core::mem::{self, MaybeUninit};
use core::slice;

use owned::Owned;
use crate::zone::*;
use crate::pointee::*;

use super::blob::*;
use super::primitive::Primitive;

/// A type whose values can be saved behind pointers in a zone.
pub unsafe trait Save<P: Ptr> : Pointee + for<'a> SaveState<'a, P> {
    fn save_poll<'a, D: Dumper<P>>(&'a self, state: &mut <Self as SaveState<'a,P>>::State, dumper: D)
        -> Result<(D, P::Persist), D::Pending>;
}

pub trait SaveState<'a, P: Ptr> {
    type State;
    fn init_save_state(&'a self) -> Self::State;
}

/// A type that can be encoded in a zone.
pub unsafe trait Encode<P: Ptr> : Sized + for<'a> SaveState<'a, P> {
    fn encode_poll<'a, D: Dumper<P>>(&'a self, state: &mut <Self as SaveState<'a,P>>::State, dumper: D)
        -> Result<D, D::Pending>;

    fn encode_blob<'a, W: WriteBlob>(&'a self, state: &<Self as SaveState<'a,P>>::State, dst: W) -> Result<W::Ok, W::Error>;
}

impl<P: Ptr, T: Primitive> SaveState<'_, P> for T {
    type State = ();
    fn init_save_state(&self) -> () {}
}

unsafe impl<P: Ptr, T: Primitive> Encode<P> for T {
    fn encode_poll<D: Dumper<P>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending>
        where P: Ptr
    {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(dst)
    }
}

unsafe impl<P: Ptr, T: Encode<P>> Save<P> for T {
    fn save_poll<'a, D: Dumper<P>>(&'a self, state: &mut <Self as SaveState<'a,P>>::State, dumper: D)
        -> Result<(D, P::Persist), D::Pending>
    {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.try_save_blob(mem::size_of::<Self>(), | dst | {
            match self.encode_blob(state, dst) {
                Ok(()) => (),
                Err(never) => never,
            }
        })
    }
}

/// Saves data to a zone.
pub trait Dumper<P: Ptr> : Sized {
    type Pending;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, P>) -> Result<P::Persist, &'p T>;

    /// Saves a blob.
    fn try_save_blob(self, size: usize, f: impl FnOnce(&mut [MaybeUninit<u8>])) -> Result<(Self, P::Persist), Self::Pending>;
}
