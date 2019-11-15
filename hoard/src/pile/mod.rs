use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;

use super::*;

use crate::marshal::{*, blob::*};

mod offset;
use self::offset::Kind;
pub use self::offset::{Offset, OffsetMut};

#[derive(Debug, Clone, Copy)]
pub struct Pile<'p> {
    marker: PhantomData<fn(&'p [u8]) -> &'p [u8]>,
    buf: &'p [u8],
}

impl<'p> Pile<'p> {
    pub unsafe fn new_unchecked(buf: &'p [u8]) -> Self {
        Self { marker: PhantomData, buf }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PileMut<'p> {
    pile: Pile<'p>,
}

impl<'p> PileMut<'p> {
    fn words(&self) -> &'p [u8] {
        self.pile.buf
    }
}

impl<'p> From<Pile<'p>> for PileMut<'p> {
    fn from(pile: Pile<'p>) -> Self {
        Self { pile }
    }
}

impl Default for Pile<'static> {
    fn default() -> Self {
        unsafe {
            Self::new_unchecked(&[])
        }
    }
}

impl Default for PileMut<'static> {
    fn default() -> Self {
        Self {
            pile: Pile::default(),
        }
    }
}

impl<'p> Zone for PileMut<'p> {
    type Ptr = OffsetMut<'p>;

    type Allocator = Self;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        Self::default()
    }
}

impl<'p> Alloc for PileMut<'p> {
    type Zone = Self;
    type Ptr = OffsetMut<'p>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata })
        })
    }

    fn zone(&self) -> Self::Zone {
        *self
    }
}

impl<'p> Get for PileMut<'p> {
    fn get<'a, T: ?Sized + Load<Self::Ptr>>(&self, own: &'a Own<T, Self::Ptr>) -> Ref<'a, T> {
        match own.raw.kind() {
            Kind::Offset(offset) => {
                /*
                let offset = usize::try_from(offset.get()).unwrap();
                let range = offset .. offset + T::blob_layout(own.metadata()).size();
                let words = self.words().get(range.clone())
                                        .unwrap_or_else(|| panic!("{:?}", range));

                let blob = Blob::<T, OffsetMut<'p>>::new(words, own.metadata()).unwrap();

                let blob = unsafe { blob.assume_fully_valid() };

                T::load_blob(blob, self)
                */
                todo!()
            },
            Kind::Ptr(ptr) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(ptr.cast().as_ptr(), own.metadata)
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T: ?Sized + Load<Self::Ptr>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
        let ptr = ptr.into_inner();

        match unsafe { ptr.raw.try_take::<T>(ptr.metadata) } {
            Ok(owned) => owned,
            Err(offset) => {
                /*
                let offset = usize::try_from(offset.get()).unwrap();
                let range = offset .. offset + T::blob_layout(metadata).size();
                let words = self.words().get(range.clone())
                                        .unwrap_or_else(|| panic!("{:?}", range));

                let blob = Blob::<T, Self>::new(words, metadata).unwrap();

                let blob = unsafe { blob.assume_fully_valid() };

                T::decode_blob(blob, self)
                */
                todo!()
            },
        }
    }
}

impl<'p> LoadPtr<OffsetMut<'p>> for PileMut<'p> {
    fn load_blob<'a, T: ?Sized + Load<OffsetMut<'p>>>(&self, offset: &'a OffsetMut<'p>, metadata: T::Metadata)
        -> FullyValidBlob<'a, T, OffsetMut<'p>>
    {
        todo!()
    }
}

#[derive(Debug)]
pub struct Tx<'p> {
    pile: Pile<'p>,
    written: Vec<u8>,
}

impl<'p> Tx<'p> {
    pub fn save<T: Save<PileMut<'p>>>(&mut self, value: &T) -> Offset<'static> {
        /*
        let mut state = value.init_save_state();

        assert!(saver.save_children(self).is_ready());

        let metadata = saver.metadata();
        let size = T::blob_layout(metadata).size();

        self.save_blob(size, |dst| {
            saver.encode_blob(dst).unwrap()
        }).unwrap()
        */
        todo!()
    }

    /*
    pub fn commit<'q: 'p, T>(&mut self, value: T, anchor: &'q mut Vec<u8>) -> (PileMut<'p>, T)
        where T: Load<PileMut<'p>>
    {
        let offset = self.save(value);

        anchor.clear();
        anchor.extend_from_slice(self.pile.words());
        anchor.extend_from_slice(&self.written);
        self.written.clear();

        unsafe {
            self.pile = Pile::new_unchecked(&anchor[..]).into();

            let pile: PileMut = Pile::new_unchecked(&anchor[..]).into();
            let ptr = OffsetMut::from_offset(offset.coerce());

            let own = Own::<T,PileMut<'p>>::from_raw_parts(ptr, T::make_sized_metadata());

            let value = pile.take(own).take_sized();
            (pile, value)
        }
    }
    */

    fn real_save_blob(&mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Offset<'static> {
        let start = self.written.len();
        self.written.resize_with(start + size, u8::default);

        let dst = &mut self.written[start..];
        f(dst);

        Offset::new(self.pile.buf.len() as u64 + start as u64).unwrap()
    }
}

impl<'p> Dumper<PileMut<'p>> for Tx<'p> {
    type Pending = !;
    type BlobPtr = Offset<'static>;

    fn save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Self::BlobPtr), !> {
        let offset = self.real_save_blob(size, f);
        Ok((self, offset))
    }
}

impl<'p> From<PileMut<'p>> for Tx<'p> {
    fn from(pile: PileMut<'p>) -> Self {
        Self {
            pile: pile.pile,
            written: vec![],
        }
    }
}

/*
use crate::bag::Bag;
pub fn test_bag<'p>(bag: &'p Bag<Bag<u8, PileMut<'p>>, PileMut<'p>>) -> Ref<'p, Bag<u8, PileMut<'p>>> {
    bag.get()
}

pub fn test_bag2<'p>(bag: &'p Bag<(u8, Option<u64>), PileMut<'p>>) -> Ref<'p, (u8, Option<u64>)> {
    bag.get()
}
*/

#[cfg(test)]
mod test {
    use super::*;

    use crate::bag::Bag;

    #[test]
    fn tx_save() {
        let v1 = Bag::<u8, PileMut>::new(1u8);
        let v2 = Bag::<u8, PileMut>::new(2u8);
        let n1 = Bag::<_, PileMut>::new((v1, v2));

        dbg!(n1);

        let tx = Tx::from(PileMut::default());
        //dbg!(tx.save(&n1));
    }

    /*
    #[test]
    fn test_commit() {
        let anchor = vec![];
        let pile = unsafe { Pile::new_unchecked(&anchor[..]) };
        let pile = PileMut::from(pile);

        let bag = Bag::new_in((12u8, 13u8), pile);
        let bag = Bag::new_in((bag, 13u8), pile);

        let mut anchor = vec![];
        let mut anchor2 = vec![];
        let mut tx = Tx::from(pile);

        {
            let (pile, bag) = tx.commit(bag, &mut anchor);

            let bag = Bag::new_in((bag, (65u8, (Some(1234u16), Bag::new_in(42u8, pile)))), pile);

            let (pile, bag) = tx.commit(bag, &mut anchor2);

            let r = bag.get();

            dbg!(pile);
            dbg!(bag.get());
        }
    }

    #[test]
    fn test_size() {
        let mut pile = PileMut::default();
        let mut tx = Tx::from(pile);

        let own = pile.alloc(42u8);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);
        let own = pile.alloc(own);

        tx.save(own);
    }

    #[test]
    fn test() {
        let mut pile = PileMut::default();
        let mut tx = Tx::from(pile);

        let own1 = pile.alloc(11u8);
        let own1 = pile.alloc(own1);

        let own2 = pile.alloc(33u8);
        let own2 = pile.alloc(own2);

        tx.save(own1);
        tx.save(own2);

        tx.save(Some(pile.alloc(Some(42u8))));
    }

    #[test]
    fn test_bag() {
        let bag = Bag::<u8, PileMut>::new(42u8);
        let mut tx = Tx::from(PileMut::default());

        tx.save(bag);
        dbg!(tx);
    }
    */
}
