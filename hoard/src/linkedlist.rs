use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ptr;

use crate::{*, marshal::{*, blob::*}};

#[derive(Debug)]
pub struct Cell<T, P: Ptr> {
    value: T,
    next: Option<OwnedPtr<Self, P>>,
}

impl<T, P: Ptr> Cell<T, P> {
    pub fn new(value: T, next: Option<OwnedPtr<Self, P>>) -> Self {
        Self { value, next }
    }

    pub fn value<'a>(self: Ref<'a, Self>) -> Ref<'a, T> {
        match self {
            Ref::Borrowed(this) => Ref::Borrowed(&this.value),
            Ref::Owned(this) => Ref::Owned(this.value),
        }
    }

    pub fn next<'a>(self: Ref<'a, Self>) -> Option<Ref<'a, OwnedPtr<Self, P>>> {
        match self {
            Ref::Borrowed(this) => {
                todo!()
            },
            Ref::Owned(this) => {
                todo!()
            },
        }
    }

    /*
    pub fn get<'a>(self: Ref<'a, Self>, mut n: usize, zone: &(impl Get<Ptr=P> + 'a)) -> Option<Ref<'a, T>> {
        let mut this = self;
        loop {
            if n == 0 {
                break Some(this.value())
            } else if let Some(next) = this.next() {
                n -= 1;
                this = zone.get_ref(next);
            } else {
                break None
            }
        }
    }
    */

    pub fn push_front(&mut self, value: T, mut alloc: impl Alloc<Ptr=P>) {
        let old_value = mem::replace(&mut self.value, value);
        let next = Self {
            value: old_value,
            next: self.next.take(),
        };

        self.next = Some(alloc.alloc(next));
    }
}

#[derive(Debug)]
pub struct CellEncodeState<T, P> {
    idx: usize,
    value_state: T,
    encode_poll_done: bool,
    next: Option<P>,
}

fn encode_cell_blob<T, P, Z, W>(value: &T, state: &T::State, ptr: &Option<P::Persist>, dst: W) -> Result<W::Ok, W::Error>
where P: Ptr,
      Z: Zone<Ptr=P>,
      T: Encode<Z>,
      W: WriteBlob,
{
    let ptr_state = <Option<P::Persist> as Encode<Z>>::init_encode_state(ptr);
    dst.write(value, state)?
       .write::<Z,_>(ptr, &ptr_state)?
       .finish()
}

unsafe impl<T, P: Ptr, Z> Encode<Z> for Cell<T, P>
where Z: Zone<Ptr=P>,
      T: Encode<Z>
{
    type State = CellEncodeState<T::State, P::Persist>;

    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT.extend(<Option<OwnedPtr<Self, P>> as Encode<Z>>::BLOB_LAYOUT);

    fn init_encode_state(&self) -> Self::State {
        let mut idx = 0;
        let mut this = self;
        loop {
            match this.next.as_ref().map(|next| P::try_get_dirty(next)) {
                Some(Ok(next)) => {
                    idx += 1;
                    this = next;
                },
                Some(Err(persist)) => break CellEncodeState {
                                                idx,
                                                value_state: this.value.init_encode_state(),
                                                encode_poll_done: false,
                                                next: Some(persist),
                },
                None => break CellEncodeState {
                                idx,
                                value_state: this.value.init_encode_state(),
                                encode_poll_done: false,
                                next: None,
                },
            }
        }
    }

    fn encode_poll<D: Dumper<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Pending> {
        let mut stack = Vec::with_capacity(state.idx);

        let mut this = self;
        for i in 0 .. state.idx {
            if let Some(Ok(next)) = this.next.as_ref().map(|next| P::try_get_dirty(next)) {
                stack.push(this);
                this = next;
            } else {
                panic!()
            }
        }


        loop {
            if !state.encode_poll_done {
                dumper = this.value.encode_poll(&mut state.value_state, dumper)?;
                state.encode_poll_done = true;
            }

            if stack.len() > 0 {
                let (new_dumper, next) = dumper.try_save_blob(Self::BLOB_LAYOUT.size(), |dst| {
                    match encode_cell_blob(&this.value, &state.value_state, &state.next, dst) {
                        Ok(()) => (),
                        Err(never) => never,
                    }
                })?;
                dumper = new_dumper;

                this = stack.pop().unwrap();
                state.idx -= 1;
                state.encode_poll_done = false;
                state.value_state = this.value.init_encode_state();
                state.next = Some(next);
            } else {
                break Ok(dumper)
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        assert_eq!(state.idx, 0);
        assert!(state.encode_poll_done);
        encode_cell_blob(&self.value, &state.value_state, &state.next, dst)
    }
}

pub fn test_save_to_vec<'s,'m>(cell: &Cell<OwnedPtr<(u8, u16, u32), pile::OffsetMut<'s,'m>>, pile::OffsetMut<'s,'m>>) -> Vec<u8> {
    pile::save_to_vec(cell)
}

pub fn test_save_to_vec2<'s,'m>(cell: &Cell<OwnedPtr<u16, pile::OffsetMut<'s,'m>>, pile::OffsetMut<'s,'m>>) -> Vec<u8> {
    pile::save_to_vec(cell)
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::pile::*;

    #[test]
    fn test_encode() {
        assert_eq!(<Cell<u8, OffsetMut> as Encode<PileMut>>::BLOB_LAYOUT.size(), 9);
        let mut alloc = PileMut::allocator();

        let mut cell = Cell::<_, OffsetMut>::new(alloc.alloc(11u8), None);

        assert_eq!(save_to_vec(&cell),
            &[11, // value
              1,0,0,0,0,0,0,0, // cell pointer
              0,0,0,0,0,0,0,0, // next
            ]);

        cell.push_front(alloc.alloc(12), &mut alloc);

        assert_eq!(save_to_vec(&cell),
            &[11, // 0th value
              1,0,0,0,0,0,0,0, // cell pointer
              0,0,0,0,0,0,0,0, // next

              12, // 1st value
              35,0,0,0,0,0,0,0, // cell pointer
              3,0,0,0,0,0,0,0   // next
            ][..]);

        let n = 1000;
        let mut cell = Cell::<_, OffsetMut>::new(alloc.alloc(42u8), None);
        for _ in 1 .. n {
            cell.push_front(alloc.alloc(42), &mut alloc);
        }

        assert_eq!(
            save_to_vec(&cell).len(),
            (n * 1) + (n * 16)
        );
    }
}
