use core::marker::PhantomData;
use core::mem;

use super::*;

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
    type PersistPtr = Offset<'static>;

    type Allocator = Self;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        Self::default()
    }

    unsafe fn dealloc_own<T: ?Sized + Pointee>(ptr: Self::Ptr, metadata: T::Metadata) {
        ptr.dealloc::<T>(metadata)
    }
}

impl<'p> Alloc for PileMut<'p> {
    type Zone = Self;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Zone> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::from_raw_parts(OffsetMut::alloc::<T>(src),
                                metadata)
        })
    }

    fn zone(&self) -> Self::Zone {
        *self
    }
}

impl<'p> Get for PileMut<'p> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, own: &'a Own<T, Self>) -> Ref<'a, T> {
        match own.ptr().kind() {
            Kind::Offset(offset) => {
                panic!("{:?}", offset)
            },
            Kind::Ptr(ptr) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(ptr.cast().as_ptr(), own.metadata())
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: Own<T, Self>) -> T::Owned {
        let (ptr, metadata) = ptr.into_raw_parts();

        match unsafe { ptr.try_take::<T>(metadata) } {
            Ok(owned) => owned,
            Err(offset) => {
                todo!()
            },
        }
    }
}

#[derive(Debug)]
pub struct Tx<'p> {
    pile: PileMut<'p>,
    written: Vec<u8>,
}

impl<'p> Tx<'p> {
    pub fn save<T: Save<PileMut<'p>>>(&mut self, value: T) -> Offset<'static> {
        let mut saver = T::save_poll(value);

        assert!(saver.save_children(self).is_ready());

        let metadata = saver.metadata();
        let size = T::blob_layout(metadata).size();

        self.save_blob(size, |dst| {
            saver.encode_blob(dst).unwrap()
        }).unwrap()
    }
}

impl<'p> From<PileMut<'p>> for Tx<'p> {
    fn from(pile: PileMut<'p>) -> Self {
        Self {
            pile,
            written: vec![],
        }
    }
}

impl<'p> PtrSaver for Tx<'p> {
    type Zone = PileMut<'p>;
    type Error = !;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [u8]))
        -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>
    {
        let start = self.written.len();
        self.written.resize(self.written.len() + size, 0xfe);

        let dst = &mut self.written[start .. start + size];

        f(dst);

        let offset = self.pile.words().len() + start;
        Ok(Offset::new(offset as u64).unwrap())
    }

    fn save_own<T: ?Sized + Save<Self::Zone>>(&mut self, own: Own<T, Self::Zone>)
        -> Result<<Self::Zone as Zone>::PersistPtr, T::SavePoll>
    {
        let (ptr, metadata) = own.into_raw_parts();
        match unsafe { ptr.try_take::<T>(metadata) } {
            Ok(owned) => Err(T::save_poll(owned)),
            Err(offset) => Ok(offset.persist()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::bag::Bag;

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
}
