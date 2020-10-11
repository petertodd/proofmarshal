use std::fmt;
use std::mem::ManuallyDrop;
use std::ptr;

use thiserror::Error;

use crate::bag::Bag;
use crate::zone::*;
use crate::blob::*;
use crate::load::*;
use crate::zone::*;
use crate::zone::heap::Heap;
use crate::owned::Ref;

#[derive(Debug)]
pub struct LinkedList<T, Z = Heap, P: Ptr = <Z as Zone>::Ptr> {
    tip: Option<Bag<Cell<T, Z, P>, Z, P>>,
    zone: Z,
}

#[derive(Debug)]
pub struct Cell<T, Z, P: Ptr = <Z as Zone>::Ptr> {
    pub value: T,
    pub next: LinkedList<T, Z, P>,
}

impl<T, Z: Default, P: Ptr> Default for LinkedList<T, Z, P> {
    fn default() -> Self {
        Self::new_in(Z::default())
    }
}

impl<T, Z, P: Ptr> Drop for LinkedList<T, Z, P> {
    fn drop(&mut self) {
        let mut tip = self.tip.take();

        while let Some(bag) = tip {
            tip = bag.try_take_dirty().ok()
                     .and_then(|cell| cell.next.into_tip());
        }
    }
}

impl<T, Z, P: Ptr> LinkedList<T, Z, P> {
    pub fn new_in(zone: Z) -> Self {
        Self {
            tip: None,
            zone,
        }
    }

    pub fn into_tip(self) -> Option<Bag<Cell<T, Z, P>, Z, P>> {
        let (tip, _zone) = self.into_raw_parts();
        tip
    }

    pub fn into_raw_parts(self) -> (Option<Bag<Cell<T, Z, P>, Z, P>>, Z) {
        let this = ManuallyDrop::new(self);
        unsafe {
            (ptr::read(&this.tip),
             ptr::read(&this.zone))
        }
    }
}

impl<T, Z: Zone> LinkedList<T, Z>
where T: Load,
      Z: AsZone<T::Zone>,
{
    pub fn push(&mut self, value: T) -> Result<(), Z::Error>
        where Z: GetMut + Alloc
    {
        let mut zone = self.zone;
        let mut this: &mut Self = self;

        while let Some(ref mut bag) = this.tip {
            let cell = bag.get_mut()?;
            this = &mut cell.next;
        }

        let new_cell = Cell::new(value, Self::new_in(zone));
        this.tip = Some(zone.alloc(new_cell));

        Ok(())
    }

    pub fn push_front(&mut self, value: T)
        where Z: Alloc
    {
        let new_next = Self {
            tip: self.tip.take(),
            zone: self.zone,
        };
        let new_cell = Cell::new(value, new_next);
        self.tip = Some(self.zone.alloc(new_cell));
    }

    pub fn get(&self, idx: usize) -> Result<Option<Ref<T>>, Z::Error>
        where Z: Get
    {
        if let Some(ref bag) = self.tip {
            match bag.get()? {
                Ref::Borrowed(cell) => {
                    cell.get(idx)
                },
                Ref::Owned(cell) => {
                    Ok(cell.take(idx)?
                           .map(Ref::Owned))
                },
            }
        } else {
            Ok(None)
        }
    }

    pub fn take(self, idx: usize) -> Result<Option<T>, Z::Error>
        where Z: Get
    {
        if let Some(bag) = self.into_tip() {
            let cell = bag.take()?;
            cell.take(idx)
        } else {
            Ok(None)
        }
    }

    pub fn get_cell(&self, idx: usize) -> Result<Option<Ref<Cell<T, Z>>>, Z::Error>
        where Z: Get
    {
        if let Some(ref bag) = self.tip {
            match bag.get()? {
                Ref::Borrowed(cell) => {
                    cell.get_cell(idx)
                },
                Ref::Owned(cell) => {
                    Ok(cell.take_cell(idx)?
                           .map(Ref::Owned))
                },
            }
        } else {
            Ok(None)
        }
    }

    pub fn take_cell(self, idx: usize) -> Result<Option<Cell<T, Z>>, Z::Error>
        where Z: Get
    {
        if let Some(bag) = self.into_tip() {
            let cell = bag.take()?;
            cell.take_cell(idx)
        } else {
            Ok(None)
        }
    }

}

impl<T, Z, P: Ptr> Cell<T, Z, P> {
    pub fn new(value: T, next: LinkedList<T, Z, P>) -> Self {
        Self {
            value,
            next,
        }
    }
}

impl<T, Z: Zone> Cell<T, Z>
where T: Load,
      Z: AsZone<T::Zone>,
{
    pub fn get(&self, idx: usize) -> Result<Option<Ref<T>>, Z::Error>
        where Z: Get
    {
        Ok(match self.get_cell(idx)? {
            None => None,
            Some(Ref::Borrowed(cell)) => Some(Ref::Borrowed(&cell.value)),
            Some(Ref::Owned(cell)) => Some(Ref::Owned(cell.value)),
        })
    }

    pub fn take(self, idx: usize) -> Result<Option<T>, Z::Error>
        where Z: Get
    {
        Ok(self.take_cell(idx)?
               .map(|cell| cell.value))
    }

    pub fn get_cell(&self, mut idx: usize) -> Result<Option<Ref<Self>>, Z::Error>
        where Z: Get
    {
        let mut this = self;

        while idx > 0 {
            if let Some(ref bag) = this.next.tip {
                match bag.get()? {
                    Ref::Borrowed(next_cell) => {
                        this = next_cell;
                        idx -= 1;
                    },
                    Ref::Owned(next_cell) => {
                        return Ok(next_cell.take_cell(idx - 1)?
                                           .map(Ref::Owned));
                    }
                }
            } else {
                return Ok(None);
            }
        }

        Ok(Some(Ref::Borrowed(this)))
    }

    pub fn take_cell(self, mut idx: usize) -> Result<Option<Self>, Z::Error>
        where Z: Get
    {
        let mut this = self;

        while idx > 0 {
            if let Some(bag) = this.next.into_tip() {
                this = bag.take()?;
                idx -= 1;
            } else {
                return Ok(None);
            }
        }

        Ok(Some(this))
    }
}

#[derive(Error)]
#[error("FIXME")]
pub enum DecodeLinkedListBytesError<T: Blob, Z: Blob, P: PtrBlob> {
    Tip(<Option<Bag<Cell<T, Z, P>, Z, P>> as Blob>::DecodeBytesError),
    Zone(Z::DecodeBytesError),
}

impl<T: Blob, Z: Blob, P: PtrBlob> fmt::Debug for DecodeLinkedListBytesError<T, Z, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeLinkedListBytesError::Tip(tip) =>
                f.debug_tuple("Tip")
                 .field(tip)
                 .finish(),
            DecodeLinkedListBytesError::Zone(zone) =>
                f.debug_tuple("Zone")
                 .field(zone)
                 .finish(),
        }
    }
}

impl<T: Blob, Z: Blob, P: PtrBlob> Blob for LinkedList<T, Z, P> {
    const SIZE: usize = <Option<Cell<T, Z, P>> as Blob>::SIZE;
    type DecodeBytesError = DecodeLinkedListBytesError<T, Z, P>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.tip)
           .write_field(&self.zone)
           .done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();
        let tip = fields.trust_field().map_err(DecodeLinkedListBytesError::Tip)?;
        let zone = fields.trust_field().map_err(DecodeLinkedListBytesError::Zone)?;
        fields.assert_done();
        Ok(Self { tip, zone }.into())
    }
}

#[derive(Error)]
#[error("FIXME")]
pub enum DecodeBlobBytesError<T: Blob, Z: Blob, P: PtrBlob> {
    Next(<LinkedList<T, Z, P> as Blob>::DecodeBytesError),
    Value(T::DecodeBytesError),
}

impl<T: Blob, Z: Blob, P: PtrBlob> fmt::Debug for DecodeBlobBytesError<T, Z, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeBlobBytesError::Value(value) =>
                f.debug_tuple("Value")
                 .field(value)
                 .finish(),
            DecodeBlobBytesError::Next(next) =>
                f.debug_tuple("Next")
                 .field(next)
                 .finish(),
        }
    }
}

impl<T: Blob, Z: Blob, P: PtrBlob> Blob for Cell<T, Z, P> {
    const SIZE: usize = T::SIZE + <Bag<LinkedList<T, Z, P>, Z, P> as Blob>::SIZE;
    type DecodeBytesError = DecodeBlobBytesError<T, Z, P>;

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.next)
           .write_field(&self.value)
           .done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = blob.struct_fields();
        let next = fields.trust_field().map_err(DecodeBlobBytesError::Next)?;
        let value = fields.trust_field().map_err(DecodeBlobBytesError::Value)?;
        fields.assert_done();
        Ok(Self { next, value }.into())
    }
}

impl<Z: Zone, P: Ptr, T: Load> Load for Cell<T, Z, P>
where Z: AsZone<T::Zone>,
{
    type Blob = Cell<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        Self {
            next: Load::load(blob.next, zone),
            value: Load::load(blob.value, zone.as_zone()),
        }
    }
}

impl<Z: Zone, P: Ptr, T: Load> Load for LinkedList<T, Z, P>
where Z: AsZone<T::Zone>,
{
    type Blob = LinkedList<T::Blob, (), P::Blob>;
    type Zone = Z;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        Self {
            tip: Load::load(blob.into_tip(), zone.as_zone()),
            zone: *zone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pile::*;

    #[test]
    fn test() -> Result<(), Box<dyn std::error::Error>> {
        let pile = PileMut::<[u8]>::default();

        let mut ll = LinkedList::new_in(pile);
        assert!(ll.get(0)?.is_none());

        for i in 0 .. 10u8 {
            ll.push(i)?;
        }

        for i in 0 .. 10 {
            if let Some(Ref::Borrowed(n)) = ll.get(i)? {
                assert_eq!(*n, i as u8);
            } else {
                panic!()
            }
        }
        assert!(ll.get(11)?.is_none());
        assert!(ll.get(12)?.is_none());

        dbg!(ll);

        Ok(())
    }

    #[test]
    fn test_drop() {
        let mut ll: LinkedList<u64> = LinkedList::default();

        for i in 0u64 .. 1_000_000 {
            ll.push_front(i);
        }
    }
}
