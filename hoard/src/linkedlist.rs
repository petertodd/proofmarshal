use core::convert::identity;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ptr;

use crate::prelude::*;
use crate::load::*;
use crate::save::*;
use crate::blob::*;
use crate::zone::*;

#[derive(Debug)]
#[repr(C)]
pub struct LinkedList<T, Z: Zone> {
    tip: Option<OwnedPtr<Cell<T,Z>, Z>>,
}

impl<T, Z: Zone> LinkedList<T, Z> {
    pub fn new() -> Own<Self, Z>
        where Z: Default
    {
        Own { this: Self::default(), zone: Z::default() }
    }

    pub fn push_front(&mut self, value: T)
        where Z: Default
    {
        let old_tip = self.tip.take();
        let new_tip = Z::allocator().alloc(Cell { value, next: old_tip });

        self.tip = Some(new_tip);
    }
}

impl<T: Load<Z>, Z: Zone> LinkedList<T, Z> {
    pub fn pop_front(mut self: RefMut<Self, Z>) -> Option<T>
        where Z: Get
    {
        match self.this.tip.take() {
            None => None,
            Some(old_tip) => {
                let old_tip = self.zone.take(old_tip).this;

                self.this.tip = old_tip.next;
                Some(old_tip.value)
            },
        }
    }

    pub fn get<'a>(self: Ref<'a, Self, Z>, n: usize) -> Option<&'a T>
        where Z: Get
    {
        match self.tip.as_ref() {
            Some(tip) => self.zone.get(tip).get(n),
            None => None,
        }
    }
}

impl<T, Z: Zone> Default for LinkedList<T,Z> {
    fn default() -> Self {
        Self { tip: None }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Cell<T, Z: Zone> {
    value: T,
    next: Option<OwnedPtr<Self, Z>>,
}

impl<T: Load<Z>, Z: Zone> Cell<T, Z> {
    pub fn get<'a>(self: Ref<'a, Self, Z>, mut n: usize) -> Option<&'a T>
        where Z: Get
    {
        let mut this = self;

        while n != 0 {
            n -= 1;
            match this.next.as_ref() {
                Some(next) => {
                    this = this.zone.get(next);
                },
                None => return None,
            }
        };

        Some(&this.value)
    }
}

impl<T, Z: Zone> Validate for LinkedList<T,Z>
where T: Validate
{
    type Error = <Option<OwnedPtr<Cell<T,Z>, Z>> as Validate>::Error;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<Option<OwnedPtr<Cell<T,Z>, Z>>,_>(identity)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T, Z: Zone> Load<Z> for LinkedList<T,Z>
where T: Load<Z>
{
    type ValidateChildren = <Option<OwnedPtr<Cell<T,Z>, Z>> as Load<Z>>::ValidateChildren;

    fn validate_children(&self) -> Self::ValidateChildren {
        self.tip.validate_children()
    }
}

#[derive(Debug)]
pub enum ValidateCellError<T, N> {
    Value(T),
    Next(N),
}

impl<T: ValidationError, N: ValidationError> ValidationError for ValidateCellError<T, N> {
}


impl<T, Z: Zone> Validate for Cell<T,Z>
where T: Validate
{
    type Error = ValidateCellError<<T as Validate>::Error,
                                   <Option<OwnedPtr<Self, Z>> as Validate>::Error>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<T,_>(ValidateCellError::Value)?;
        blob.field::<Option<OwnedPtr<Self, Z>>,_>(ValidateCellError::Next)?;
        unsafe { blob.assume_valid() }
    }
}

pub struct CellValidator<T: Load<Z>, Z: Zone> {
    value: T::ValidateChildren,
    next: Option<FatPtr<Cell<T, Z>, Z::Persist>>,
}

unsafe impl<T, Z: Zone> Load<Z> for Cell<T,Z>
where T: Load<Z>
{
    type ValidateChildren = CellValidator<T, Z>;

    fn validate_children(&self) -> Self::ValidateChildren {
        CellValidator {
            value: self.value.validate_children(),
            next: self.next.as_ref()
                      .and_then(|ptr| Z::try_get_dirty(ptr).err())
        }
    }
}

impl<T, Z: Zone> ValidateChildren<Z> for CellValidator<T,Z>
where T: Load<Z>
{
    fn poll<V: PtrValidator<Z>>(&mut self, ptr_validator: &V) -> Result<(), V::Error> {
        loop {
            self.value.poll(ptr_validator)?;

            match self.next.as_ref() {
                Some(next) => {
                    match ptr_validator.validate_ptr(next)? {
                        Some(next) => {
                            *self = next;
                        },
                        None => break Ok(()),
                    }
                },
                None => break Ok(()),
            }
        }
    }
}

impl<T, Z: Zone, Y: Zone> Encoded<Y> for Cell<T, Z>
where T: Encoded<Y>,
{
    type Encoded = Cell<T::Encoded, Y>;
}

pub enum SaveCellState<'a, T: Encode<'a, Y>, Z: Zone, Y: Zone> {
    Initial(&'a Cell<T, Z>),
    Poll {
        stack: Vec<&'a T>,

        value: &'a T,
        value_state: T::State,

        next: Option<<Y::Persist as Zone>::Ptr>,
    },
}

fn encode_cell_blob<'a, T, Z, Y, W>(
    value: &'a T,
    value_state: &T::State,
    next: &Option<<Y::Persist as Zone>::Ptr>,
    dst: W,
) -> Result<W::Ok, W::Error>
where T: 'a + Encode<'a, Y>,
      Z: Zone, Y: Zone,
      W: WriteBlob,
{
    todo!()
}

impl<'a, T: 'a, Z: Zone, Y: Zone> Encode<'a, Y> for Cell<T, Z>
where T: Encode<'a, Y>, Z: Encode<'a, Y>
{
    type State = SaveCellState<'a, T, Z, Y>;

    fn save_children(&'a self) -> Self::State {
        SaveCellState::Initial(self)
    }

    fn poll<D: Dumper<Y>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        if let SaveCellState::Initial(this) = state {
            let mut stack = vec![];

            let next = loop {
                match this.next.as_ref().map(|next| Z::zone_save_ptr(next, &dumper)) {
                    None => break None,
                    Some(Ok(next_ptr)) => break Some(dumper.coerce_ptr(next_ptr)),
                    Some(Err(next_cell)) => {
                        stack.push(&this.value);
                        *this = next_cell;
                    },
                }
            };

            *state = SaveCellState::Poll {
                stack,
                value_state: this.value.save_children(),
                value: &this.value,
                next,
            }
        };

        if let SaveCellState::Poll { stack, value, value_state, next } = state {
            loop {
                dumper = value.poll(value_state, dumper)?;

                if stack.len() > 1 {
                    let (d, new_next) = dumper.save_blob(mem::size_of::<Self::Encoded>(), |dst| {
                        encode_cell_blob::<T,Z,Y,_>(value, value_state, next, dst)
                    })?;
                    dumper = d;

                    *value = stack.pop().unwrap();
                    *value_state = value.save_children();
                    *next = Some(dumper.coerce_ptr(new_next));
                } else {
                    break Ok(dumper)
                }
            }
        } else {
            unreachable!()
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let SaveCellState::Poll { stack, value, value_state, next } = state {
            assert_eq!(stack.len(), 0, "poll() unfinished");

            encode_cell_blob::<T, Z, Y, W>(value, value_state, next, dst)
        } else {
            panic!("poll() unfinished")
        }
    }
}


use crate::pile::*;
pub fn test_linkedlist_validate<'p,'v>(
    pile: Pile<'p,'v>,
    list: &LinkedList<bool, Pile<'p, 'v>>,
) -> Result<(), crate::pile::error::ValidatorError<'p,'v>>
{
    let mut validator = list.validate_children();
    validator.poll(&pile)
}

use crate::pile::*;
pub fn test_linkedlist_validate2<'p,'v>(
    pile: Pile<'p,'v>,
    list: &LinkedList<LinkedList<bool, Pile<'p,'v>>, Pile<'p, 'v>>,
) -> Result<(), crate::pile::error::ValidatorError<'p,'v>>
{
    let mut validator = list.validate_children();
    validator.poll(&pile)
}

pub fn test_linkedlist_save_children<'a, 'p, 'v>(list: &'a Cell<bool, Pile<'p,'v>>,
) -> SaveCellState<'a, bool, Pile<'p,'v>, Pile<'p,'v>>
{
    Save::save_children(list)
}

pub fn test_linkedlist_save_children_mut<'a, 'p, 'v>(list: &'a Cell<ValidPtr<u8, PileMut<'p,'v>>, PileMut<'p,'v>>,
) -> SaveCellState<'a, ValidPtr<u8, PileMut<'p,'v>>, PileMut<'p,'v>, Pile<'p,'v>>
{
    Save::save_children(list)
}



#[cfg(test)]
mod tests {
    use super::*;

    use crate::pile::*;

    #[test]
    fn linkedlist_push() {
        let mut l = LinkedList::<u8, PileMut>::new();

        for i in 0 .. 100 {
            l.push_front(i);
        }

        for i in 0 .. 100 {
            let n = l.as_ref().get(i);
            let expected = 99 - i as u8;
            assert_eq!(n.copied(), Some(expected));
        }

        for i in 0 .. 100 {
            let n = l.as_mut().pop_front().unwrap();
            let expected = 99 - i as u8;
            assert_eq!(n, expected);
        }
    }

    #[test]
    fn linkedlist_default() {
        let _ = LinkedList::<u8, Pile>::default();
    }
}
