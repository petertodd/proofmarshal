use core::cell::Cell;

pub mod offset;

/// An in-memory stack of blobs
#[derive(Debug)]
pub struct Pile<'a> {
    chunks: &'a Chunks,
}

#[derive(Debug)]
struct Chunks {
    buf: Vec<u8>,
    offset: Cell<usize>,
    capacity: usize,
}

impl Pile<'_> {
    pub fn open<R>(f: impl FnOnce(Pile<'_>) -> R) -> R {
        let capacity = 10000;

        let chunks = Chunks {
            buf: Vec::with_capacity(capacity),
            offset: 0.into(),
            capacity,
        };

        f(Pile { chunks: &chunks })
    }
}

/*
#[derive(Debug)]
pub struct OutOfRange;

impl<'a> Arena for Pile<'a> {
    type Ptr = Offset<'a>;
    type Error = OutOfRange;

    fn try_deref_ptr<'p, T: ?Sized + Load<Self>>(&self, ptr: &'p Ptr<T, Self::Ptr>) -> Result<Ref<'p, T>, Self::Error> {
        unimplemented!()
    }
}

impl<'a> blob::Arena for Pile<'a> {
    type Offset = Offset<'a>;
    type OffsetError = OutOfRange;

    fn validate_offset<'p, T: ?Sized>(&self, offset: &'p Ptr<T, Self::Offset>) -> Result<&'p Ptr<T, Self::Ptr>, Self::OffsetError>
        where T: Persist<Self>
    {
        unimplemented!()
    }
}

impl<'a> Alloc<Pile<'a>> for Pile<'a> {
    fn alloc<T: Store<Pile<'a>>>(&mut self, value: T) -> Own<T,Pile<'a>> {
        let (offset, metadata) = <T as Store<Pile<'a>>>::store_blob(value.to_owned(), self);

        let ptr = unsafe { Ptr::new(offset, metadata) };

        let new_pile = Pile { chunks: self.chunks };
        Own::from_ptr(ptr, new_pile)
    }
}

impl<'a> AllocBlob<Pile<'a>> for Pile<'a> {
    fn alloc_bytes(&mut self, len: usize, init_fn: impl FnOnce(&mut [u8])) -> Offset<'a> {
        let cur_offset = self.chunks.offset.get();
        assert!(cur_offset <= self.chunks.capacity);

        let dst: &mut [u8] = unsafe {
            let start = self.chunks.buf.as_ptr() as *const u8 as *mut u8;
            self.chunks.offset.set(cur_offset + len);

            core::slice::from_raw_parts_mut(start.offset(cur_offset as isize), len)
        };

        init_fn(dst);

        unsafe {
            Offset::new_unchecked(cur_offset as u64)
        }
    }
}




#[cfg(test)]
mod tests {
    use super::*;

    use crate::tuple::Item;

    #[test]
    fn test() {
        Pile::open(|mut pile| {
            for i in 0 .. 255 {
                pile.alloc(1u8);
            }
            dbg!(pile);

            let tup = Item(1u8, Item(2u8, ()));

            //let owned = pile.alloc(tup);
        })
    }
}
*/
