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

use crate::collections::leaf::Leaf;
use crate::collections::length::*;
use crate::collections::perfecttree::PerfectTree;

pub mod peaktree;
use self::peaktree::PeakTree;

#[derive(Debug)]
pub struct MMR<T, Z = (), P: Ptr = <Z as Zone>::Ptr> {
    peaks: Option<PeakTree<T, Z, P>>,
    zone: Z,
}

impl<T, Z: Zone> Default for MMR<T, Z>
where Z: Default
{
    fn default() -> Self {
        Self::new_in(Z::default())
    }
}

impl<T, Z: Zone> MMR<T, Z> {
    pub fn new_in(zone: Z) -> Self {
        Self {
            peaks: None,
            zone,
        }
    }

    pub fn len(&self) -> Length {
        self.peaks.as_ref()
            .map(|peaks| {
                peaks.len().into()
            }).unwrap_or(Length(0))
    }
}

impl<T, Z: Zone> MMR<T, Z>
where T: Load,
{
    pub fn try_push(&mut self, value: T) -> Result<(), T>
        where Z: GetMut + Alloc
    {
        if self.len() < Length::MAX {
            let leaf = Leaf::new_in(value, self.zone);
            match self.try_push_leaf(leaf) {
                Ok(()) => Ok(()),
                Err(_overflow) => unreachable!("overflow condition already checked"),
            }
        } else {
            Err(value)
        }
    }

    pub fn try_push_leaf(&mut self, leaf: Leaf<T, Z>) -> Result<(), Leaf<T, Z>>
        where Z: GetMut + Alloc
    {
        if self.len() < Length::MAX {
            let new_peak = if let Some(peaks) = self.peaks.take() {
                peaks.try_push_peak(leaf.into()).ok().expect("overflow condition already checked")
            } else {
                PeakTree::from(PerfectTree::from(leaf))
            };
            self.peaks = Some(new_peak);
            Ok(())
        } else {
            Err(leaf)
        }
    }

    /*
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
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::zone::heap::Heap;

    #[test]
    fn test_push() {
        let mut mmr = MMR::<u32,Heap>::default();

        dbg!(&mmr);

        for i in 1 ..= 10240 {
            mmr.try_push(i).unwrap();
            eprintln!("mmr.len() = 0b{:b}", mmr.len());
        }
        dbg!(mmr.len().split());
    }
}
