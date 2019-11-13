use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ptr;

use crate::{*, marshal::{*, blob::*}};

#[derive(Debug)]
pub struct LinkedList<T, Z: Zone> {
    tip: Option<Own<Cell<T,Z>, Z>>,
    zone: Z::Allocator,
}

impl<T, Z: Zone> LinkedList<T,Z> {
    pub fn new_in(alloc: Z::Allocator) -> Self {
        Self {
            tip: None,
            zone: alloc,
        }
    }

    pub fn push_front(&mut self, value: T) {
        let new_cell = Cell::new(value, self.tip.take());
        self.tip = Some(self.zone.alloc(new_cell));
    }
}

impl<T, Z: Zone> LinkedList<T,Z>
where T: Load<Z> + Owned<Owned=T>,
      Z: Save<Z>,
{
    pub fn pop_front(&mut self) -> Option<T>
        where Z: Get,
    {
        match self.tip.take() {
            None => None,
            Some(tip) => {
                let (value, next) = self.zone.zone().take(tip).split();
                self.tip = next;
                Some(value)
            },
        }
    }

    pub fn front(&self) -> Option<Ref<T>>
        where Z: Get
    {
        match &self.tip {
            None => None,
            Some(tip) => Some(
                match self.zone.zone().get(tip) {
                    Ref::Borrowed(cell) => Ref::Borrowed(&cell.value),
                    Ref::Owned(cell) => Ref::Owned(cell.into_value()),
                }
            )
        }
    }

    pub fn get(&self, mut idx: usize) -> Option<Ref<T>>
        where Z: Get
    {
        if let Some(tip) = &self.tip {
            let tip = self.zone.zone().get(tip);
            loop {
                if idx == 0 {
                    break Some(match tip {
                        Ref::Borrowed(cell) => Ref::Borrowed(&cell.value),
                        Ref::Owned(cell) => Ref::Owned(cell.into_value()),
                    })
                }

                /*
                if let Some(own_cell) = match tip {
                    Ref::Borrowed(cell) => Ref::Borrowed(&cell.next),
                    Ref::Owned(cell) => Ref::Borrowed(cell.value),
                } {
                }
                */
                /*
                match tip {
                    None => break None,
                    Some(tip) if idx == 0 => break Some(
                        match self.zone.zone().get(tip) {
                            Ref::Borrowed(cell) => Ref::Borrowed(&cell.value),
                            Ref::Owned(cell) => Ref::Owned(cell.into_value()),
                        }
                    ),
                    Some(tip) => {
                        todo!()
                    }
                }
                */
            }
        } else {
            None
        }
    }
}

pub struct LinkedListSavePoll<T, Z, Y> {
    marker: PhantomData<(T,Z,Y)>,
}

impl<T, Z: Zone, Y: Zone> Save<Y> for LinkedList<T,Z>
where T: Save<Y>,
      Z: Save<Y>,
{
    const BLOB_LAYOUT: BlobLayout = <Option<Own<Cell<T,Z>, Z>> as Save<Y>>::BLOB_LAYOUT;

    type SavePoll = LinkedListSavePoll<T,Z,Y>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        todo!()
    }
}

impl<T, Z: Zone, Y: Zone> SavePoll<Y> for LinkedListSavePoll<T,Z,Y>
where T: Save<Y>,
      Z: Save<Y>,
{
    type Target = LinkedList<T,Z>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Y>
    {
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}


#[derive(Debug)]
pub struct Cell<T, Z: Zone> {
    value: T,
    next: Option<Own<Self,Z>>,
}

impl<T, Z: Zone> Drop for Cell<T,Z> {
    fn drop(&mut self) {
        while let Some(next_own) = self.next.take() {
            if let Some(cell) = Z::drop_take(next_own) {
                self.next = cell.into_next();
            }
        }
    }
}

impl<T, Z: Zone> Cell<T,Z> {
    pub fn new(value: T, next: Option<Own<Self,Z>>) -> Self {
        Self { value, next }
    }

    pub fn split(self) -> (T, Option<Own<Self,Z>>) {
        let this = ManuallyDrop::new(self);
        unsafe {
            (ptr::read(&this.value),
             ptr::read(&this.next))
        }
    }

    pub fn into_value(self) -> T {
        let (value, _) = self.split();
        value
    }

    pub fn into_next(self) -> Option<Own<Self,Z>> {
        let (_, next) = self.split();
        next
    }
}

impl<T, Z: Zone, Y: Zone> Save<Y> for Cell<T,Z>
where T: Save<Y>,
      Z: Save<Y>,
{
    const BLOB_LAYOUT: BlobLayout = <T as Save<Y>>::BLOB_LAYOUT.extend(<Option<Own<Self,Z>> as Save<Y>>::BLOB_LAYOUT);

    type SavePoll = CellSavePoll<T,Z,Y>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        todo!()
    }
}

pub struct CellSavePoll<T, Z, Y> {
    marker: PhantomData<(T,Z,Y)>,
}

impl<T, Z: Zone, Y: Zone> SavePoll<Y> for CellSavePoll<T,Z,Y>
where T: Save<Y>,
      Z: Save<Y>,
{
    type Target = Cell<T,Z>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Y>
    {
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}

pub struct CellError;

impl<T, Z: Zone> Load<Z> for Cell<T,Z>
where T: Load<Z>,
      Z: Save<Z>,
{
    type Error = CellError;

    type ValidateChildren = ();
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        todo!()
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self::Owned {
        todo!()
    }
}

pub fn test_front(list: &LinkedList<u64,crate::heap::Heap>) -> Option<Ref<u64>> {
    list.front()
}

pub fn test_pop_front(list: &mut LinkedList<u64,crate::heap::Heap>) -> Option<u64> {
    list.pop_front()
}

pub fn test_push_front(list: &mut LinkedList<u64,crate::heap::Heap>, value: u64) {
    list.push_front(value)
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::heap::Heap;

    #[test]
    fn test() {
        let mut list = LinkedList::<u8,Heap>::new_in(Heap);

        for i in 0 .. 10 {
            list.push_front(i);

            assert_eq!(*list.front().unwrap(), i);
        }

        for i in (0 .. 10).rev() {
            assert_eq!(list.pop_front(), Some(i));
        }
        assert_eq!(list.pop_front(), None);
    }

    #[test]
    fn test_drop() {
        let mut list = LinkedList::<_,Heap>::new_in(Heap);

        for i in 0 .. 100_000 {
            list.push_front(i)
        }
    }
}
