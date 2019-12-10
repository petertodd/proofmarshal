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
    values: Vec<T>,
    end: Option<P>,
}

unsafe impl<T, P: Ptr, Z> Encode<Z> for Cell<T, P>
where Z: Zone<Ptr=P>,
      T: Encode<Z>
{
    type State = CellEncodeState<T::State, P::Persist>;

    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT.extend(<Option<OwnedPtr<Self, P>> as Encode<Z>>::BLOB_LAYOUT);

    fn init_encode_state(&self) -> Self::State {
        let mut values = vec![];

        let mut this = self;
        loop {
            values.push(this.value.init_encode_state());

            match this.next.as_ref().map(|next| P::try_get_dirty(next)) {
                Some(Ok(next)) => {
                    this = next;
                },
                Some(Err(persist)) => break CellEncodeState { values, end: Some(persist) },
                None => break CellEncodeState { values, end: None },
            }
        }
    }

    fn encode_poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending> {
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::pile::*;

    #[test]
    fn test_encode() {
        assert_eq!(<Cell<u8, OffsetMut> as Encode<PileMut>>::BLOB_LAYOUT.size(), 10);
        let mut alloc = PileMut::allocator();

        let mut cell = Cell::<_, OffsetMut>::new(alloc.alloc(0), None);

        for i in 1 .. 10 {
            cell.push_front(alloc.alloc(i), &mut alloc);
        }

        let state = Encode::<PileMut>::init_encode_state(&cell);
        dbg!(mem::size_of_val(&state));
        dbg!(&state);
    }
}
