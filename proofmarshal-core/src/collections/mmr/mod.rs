use std::cell::Cell;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::error;
use std::borrow::{Borrow, BorrowMut};
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops::DerefMut;
use std::ptr;

use thiserror::Error;

use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use hoard::bag::Bag;
use hoard::primitive::Primitive;
use hoard::owned::{IntoOwned, Take, Ref, Own};
use hoard::pointee::Pointee;
use hoard::zone::{Alloc, Get, GetMut, Ptr, PtrBlob, Zone};
use hoard::load::{Load, LoadRef, MaybeValid};

use crate::collections::perfecttree::Leaf;
use crate::collections::length::*;

pub mod peaktree;
use self::peaktree::PeakTree;

pub struct MMR<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    state: State<T, Z, P>,
    zone: Z,
}

enum State<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    Empty,
    Peaks(PeakTree<T, Z, P>),
}

impl<T, Z: Zone> Default for MMR<T, Z>
where Z: Default
{
    fn default() -> Self {
        Self::new_in(Z::default())
    }
}

impl<T, Z: Zone> MMR<T, Z> {
    pub fn new_in(zone: impl Borrow<Z>) -> Self {
        Self {
            state: State::Empty,
            zone: zone.borrow().clone(),
        }
    }

    pub fn len(&self) -> Length {
        match &self.state {
            State::Empty => Length(0),
            State::Peaks(peaks) => peaks.len().into(),
        }
    }
}

#[derive(Debug)]
pub struct OverflowError<T>(pub T);

impl<T, Z: Zone> MMR<T, Z>
where T: Load,
{
    pub fn push(&mut self, value: T) -> Result<Result<(), OverflowError<T>>, Z::Error>
        where Z: Alloc
    {
        if self.len() < Length::MAX {
            let leaf = Leaf::new_in(value, self.zone);
            Ok(match self.push_leaf(leaf)? {
                Ok(()) => Ok(()),
                Err(_overflow) => unreachable!("overflow condition already checked"),
            })
        } else {
            Ok(Err(OverflowError(value)))
        }
    }

    pub fn push_leaf(&mut self, leaf: Leaf<T, Z>) -> Result<Result<(), OverflowError<Leaf<T, Z>>>, Z::Error>
        where Z: Alloc
    {
        todo!()
    }

    pub fn get(&self, _idx: usize) -> Result<Option<Ref<T>>, Z::Error>
        where Z: Get
    {
        todo!()
    }

    pub fn get_leaf(&self, _idx: usize) -> Result<Option<Ref<Leaf<T, Z>>>, Z::Error>
        where Z: Get
    {
        match &self.state {
            State::Empty => Ok(None),
            State::Peaks(_peaks) => todo!(), //tree.get_leaf(idx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test() {
        let mmr = MMR::<u8,Heap>::default();
    }
}
