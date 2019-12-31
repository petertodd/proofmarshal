use std::mem;

use crate::pointee::Pointee;
use crate::marshal::blob::WriteBlob;
use crate::zone::{Zone, ValidPtr};

use super::{Dumper, encode::*};

pub trait Saved<Y> : Pointee {
    type Saved : ?Sized + Pointee<Metadata = Self::Metadata>;
}

impl<Y, T: Encoded<Y>> Saved<Y> for T {
    type Saved = T::Encoded;
}

pub trait Save<'a, Y> : Saved<Y> {
    type State;
    fn make_save_state(&'a self) -> Self::State;

    fn save_poll<D>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Error>
        where D: Dumper<Y>;
}

impl<'a, Y, T: Encode<'a, Y>> Save<'a, Y> for T {
    type State = T::State;

    fn make_save_state(&'a self) -> Self::State {
        self.make_encode_state()
    }

    fn save_poll<D>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Error>
        where D: Dumper<Y>
    {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.encode_value(self, state)
    }
}

pub trait SavePtr<Z: Zone> : Zone {
    fn try_save_ptr<'a, T, D>(ptr: &'a ValidPtr<T, Self>, dumper: &D) -> Result<Z::PersistPtr, &'a T>
        where T: ?Sized + Pointee,
              D: Dumper<Z>;
}
