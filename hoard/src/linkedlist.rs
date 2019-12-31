use std::convert;
use std::fmt;
use std::mem::{self, ManuallyDrop};
use std::ptr;

use crate::prelude::*;
use crate::zone::FatPtr;

use crate::marshal::*;
use crate::marshal::blob::*;
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::save::*;

use thiserror::Error;

#[derive(Debug)]
pub struct LinkedList<T, Z: Zone> {
    tip: Option<OwnedPtr<Cell<T, Z>, Z>>,
}

impl<T, Z: Zone> LinkedList<T,Z> {
    pub fn new() -> Self {
        Self { tip: None }
    }

    /// Prepends the list with a value.
    pub fn push_front(&mut self, value: T, zone: &Z)
        where Z: Alloc
    {
        let old_tip = self.tip.take();
        self.tip = Some(zone.alloc(Cell::new(value, old_tip)));
    }
}

impl<T: 'static, Z: 'static + Zone> ValidateBlob for LinkedList<T, Z>
where T: ValidateBlob
{
    type Error = <Option<OwnedPtr<Cell<T,Z>, Z>> as ValidateBlob>::Error;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.field::<Option<OwnedPtr<Cell<T, Z>, Z>>,_>(convert::identity)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T, Z: Zone> Persist for LinkedList<T, Z>
where T: Persist,
{
    type Persist = LinkedList<T::Persist, Z::Persist>;
    type Error = <Self::Persist as ValidateBlob>::Error;
}


unsafe impl<'a, T, Z: Zone> ValidateChildren<'a, Z> for LinkedList<T, Z>
where T: ValidateChildren<'a, Z>,
{
    type State = <Option<OwnedPtr<Cell<T, Z>, Z>> as ValidateChildren<'a, Z>>::State;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        <Option<OwnedPtr<Cell<T, Z>, Z>> as ValidateChildren<'a, Z>>::validate_children(&this.tip)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        <Option<OwnedPtr<Cell<T, Z>, Z>> as ValidateChildren<'a, Z>>::poll(&this.tip, state, validator)
    }
}

impl<Z: Zone, T: Decode<Z>> Decode<Z> for LinkedList<T,Z> {
}

impl<Y: Zone, T: Encoded<Y>, Z: Zone> Encoded<Y> for LinkedList<T,Z> {
    type Encoded = LinkedList<T::Encoded, Y>;
}

impl<'a, Y: Zone, T: 'a + Encode<'a, Y>, Z: 'a + Zone> Encode<'a, Y> for LinkedList<T, Z>
where Z: SavePtr<Y>
{
    type State = <Option<OwnedPtr<Cell<T, Z>, Z>> as Encode<'a, Y>>::State;

    fn make_encode_state(&'a self) -> Self::State {
        self.tip.make_encode_state()
    }

    fn encode_poll<D: Dumper<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        self.tip.encode_poll(state, dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        self.tip.encode_blob(state, dst)
    }
}



#[derive(Debug)]
#[repr(C)]
pub struct Cell<T, Z: Zone> {
    value: T,
    next: Option<OwnedPtr<Self, Z>>,
}

impl<T, Z: Zone> Drop for Cell<T,Z> {
    fn drop(&mut self) {
        // Need to implement drop ourselves because Rust's default drop will blow the stack.
        let mut next = self.next.take();
        while let Some(next_ptr) = next.take() {
            if let Ok((_, new_next)) = Z::try_take_dirty(next_ptr).map(Cell::into_raw_parts) {
                next = new_next;
            }
        }
    }
}

impl<T, Z: Zone> Cell<T, Z> {
    pub fn new(value: T, next: Option<OwnedPtr<Self, Z>>) -> Self {
        Self { value, next }
    }

    pub fn into_raw_parts(self) -> (T, Option<OwnedPtr<Self, Z>>) {
        let this = ManuallyDrop::new(self);
        unsafe { (ptr::read(&this.value), ptr::read(&this.next)) }
    }
}


impl<T: 'static, Z: 'static + Zone> ValidateBlob for Cell<T, Z>
where T: ValidateBlob
{
    type Error = ValidateCellError<<Option<OwnedPtr<Self, Z>> as ValidateBlob>::Error,
                                   <T as ValidateBlob>::Error>;

    fn validate<'a, V>(mut blob: BlobCursor<'a, Self, V>) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
        where V: PaddingValidator
    {
        blob.field::<T,_>(ValidateCellError::Value)?;
        blob.field::<Option<OwnedPtr<Self, Z>>,_>(ValidateCellError::Next)?;
        unsafe { blob.assume_valid() }
    }
}

#[derive(Debug, Error)]
#[error("cell")]
pub enum ValidateCellError<Next: fmt::Debug, Value: fmt::Debug> {
    Next(Next),
    Value(Value),
}

unsafe impl<T, Z: Zone> Persist for Cell<T, Z>
where T: Persist,
{
    type Persist = Cell<T::Persist, Z::Persist>;
    type Error = <Self::Persist as ValidateBlob>::Error;
}

#[derive(Debug)]
pub struct CellValidator<'a, T: ValidateChildren<'a, Z>, Z: Zone> {
    value: &'a T::Persist,
    value_state: T::State,
    next: Option<&'a OwnedPtr<Cell<T::Persist, Z::Persist>, Z::Persist>>,
}

unsafe impl<'a, T, Z: Zone> ValidateChildren<'a, Z> for Cell<T, Z>
where T: ValidateChildren<'a, Z>,
{
    type State = Option<CellValidator<'a, T, Z>>;

    fn validate_children(this: &'a Self::Persist) -> Self::State {
        None
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        loop { match state {
            None => {
                *state = Some(CellValidator {
                    value: &this.value,
                    value_state: T::validate_children(&this.value),
                    next: this.next.as_ref(),
                });
            },
            Some(state) => {
                T::poll(&this.value, &mut state.value_state, validator)?;

                if let Some(next_ptr) = state.next {
                    if let Some(next_cell) = validator.validate_ptr::<Self>(next_ptr)? {
                        state.value = &next_cell.value;
                        state.value_state = T::validate_children(&state.value);
                        state.next = next_cell.next.as_ref();
                    } else {
                        // While we're not at the end of the list, the validator doesn't need us to
                        // validate the next cell, so we're done.
                        break Ok(())
                    }
                } else {
                    // There isn't another cell to validate, as we're at the end of the list
                    break Ok(())
                }
            },
        }}
    }
}

impl<Z: Zone, T: Decode<Z>> Decode<Z> for Cell<T,Z> {
}

impl<Y: Zone, T: Encoded<Y>, Z: Zone> Encoded<Y> for Cell<T,Z> {
    type Encoded = Cell<T::Encoded, Y>;
}


pub enum CellEncoder<'a, Y: Zone, T: Encode<'a, Y>, Z: Zone> {
    Initial(&'a Cell<T, Z>),
    Poll {
        stack: Vec<&'a T>,
        value: &'a T,
        value_state: T::State,
        next: Option<Y::PersistPtr>,
    },
}

#[inline(always)]
fn encode_cell_blob_impl<'a, Y: Zone, T: Encode<'a, Y>, W: WriteBlob>(
    value: &'a T,
    value_state: &T::State,
    next: &Option<Y::PersistPtr>,
    dst: W,
) -> Result<W::Ok, W::Error>
{
    dst.write(value, value_state)?
       .write_primitive(next)?
       .finish()
}

impl<'a, Y: Zone, T: 'a + Encode<'a, Y>, Z: 'a + Zone> Encode<'a, Y> for Cell<T, Z>
where Z: SavePtr<Y>
{
    type State = CellEncoder<'a, Y, T, Z>;

    fn make_encode_state(&'a self) -> Self::State {
        CellEncoder::Initial(self)
    }

    fn encode_poll<D: Dumper<Y>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        loop { match state {
            CellEncoder::Initial(first_cell) => {
                let mut this: &Self = first_cell;
                let mut stack = vec![];

                let next = loop {
                    match this.next.as_ref().map(|next| Z::try_save_ptr(next, &dumper)) {
                        // We're at the end of the list
                        None => break None,

                        // We're not at the end of the list, but the next cell has already been
                        // saved.
                        Some(Ok(next_ptr)) => break Some(next_ptr),

                        // There's another dirty cell that needs saving
                        Some(Err(next_cell)) => {
                            stack.push(&this.value);
                            this = next_cell;
                        },
                    }
                };

                *state = CellEncoder::Poll {
                    stack,
                    value: &this.value,
                    value_state: this.value.make_encode_state(),
                    next,
                };
            },
            CellEncoder::Poll { stack, value, value_state, next } => {
                dumper = value.encode_poll(value_state, dumper)?;

                if stack.len() > 0 {
                    let (d, new_next) = dumper.save_blob(mem::size_of::<Self::Encoded>(), |dst| {
                        encode_cell_blob_impl(*value, value_state, next, dst)
                    })?;
                    dumper = d;

                    *value = stack.pop().unwrap();
                    *value_state = value.make_encode_state();
                    *next = Some(D::blob_ptr_to_zone_ptr(new_next));
                } else {
                    break Ok(dumper)
                }
            },
        }}
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        match state {
            CellEncoder::Poll { stack, value, value_state, next } if stack.len() == 0 => {
                encode_cell_blob_impl(*value, value_state, next, dst)
            },
            _ => panic!("poll() unfinished"),
        }
    }
}

use crate::pile::*;
pub fn test_encode<'p,'v>(
    pile: TryPileMut<'p,'v>,
    list: &LinkedList::<LinkedList<u8, TryPileMut<'p,'v>>, TryPileMut<'p,'v>>
) -> Vec<u8>
{
    pile.encode_dirty(list)
}

pub fn test_drop<'p,'v>(
    _list: LinkedList::<LinkedList<u8, TryPileMut<'p,'v>>, TryPileMut<'p,'v>>
)
{
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::pile::*;

    #[test]
    fn linkedlist_u8_encode() {
        let pile = &TryPileMut::default();
        let mut l = LinkedList::<u8, TryPileMut>::new();

        for i in 0 .. 4 {
            l.push_front(i, pile);
        }

        assert_eq!(pile.encode_dirty(&l),
                   &[// value, Option<next>
                     0,   0,0,0,0,0,0,0,0,
                     1,   1,0,0,0,0,0,0,0,
                     2,  19,0,0,0,0,0,0,0,
                     3,  37,0,0,0,0,0,0,0,

                     // tip
                     55,0,0,0,0,0,0,0,
                   ][..]);
    }

    #[test]
    pub fn big_linkedlist_u64_encode() {
        let pile = &TryPileMut::default();
        let mut l = LinkedList::<Le<u64>, TryPileMut>::new();

        for i in 0 .. 100_000 {
            l.push_front(i.into(), pile);
        }

        assert_eq!(pile.encode_dirty(&l).len(), 8 + 1_600_000);
    }

    #[test]
    fn linkedlist_meta_encode() {
        let pile = &TryPileMut::default();
        let mut l = LinkedList::<LinkedList<u8, TryPileMut>, TryPileMut>::new();

        for _ in 0 .. 3 {
            let mut l2 = LinkedList::new();
            for i in 0 .. 3 {
                l2.push_front(i + 50, pile);
            }
            l.push_front(l2, pile);
        }

        assert_eq!(pile.encode_dirty(&l),
                   &[
                     50,   0,0,0,0,0,0,0,0,
                     51,   1,0,0,0,0,0,0,0,
                     52,  19,0,0,0,0,0,0,0,

                     37,0,0,0,0,0,0,0,     0,0,0,0,0,0,0,0,

                     50,   0,0,0,0,0,0,0,0,
                     51,  87,0,0,0,0,0,0,0,
                     52, 105,0,0,0,0,0,0,0,

                     123,0,0,0,0,0,0,0,   55,0,0,0,0,0,0,0,

                     50,   0,0,0,0,0,0,0,0,
                     51, 173,0,0,0,0,0,0,0,
                     52, 191,0,0,0,0,0,0,0,

                     209,0,0,0,0,0,0,0,  141,0,0,0,0,0,0,0,

                     227,0,0,0,0,0,0,0,
                   ][..]);
    }
}
