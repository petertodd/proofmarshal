use core::marker::PhantomData;
use core::mem;

use super::*;

mod offset;
pub use self::offset::Offset;

#[derive(Debug, Clone, Copy)]
pub struct Pile<'p> {
    marker: PhantomData<fn(&'p [u8]) -> &'p [u8]>,
    buf: &'p [u8],
}

impl<'p> Pile<'p> {
    pub fn new(buf: &'p [u8]) -> Self {
        Self { marker: PhantomData, buf }
    }
}

impl<'p> Zone for Pile<'p> {
    type Ptr = Offset<'p>;
    type PersistPtr = Offset<'static>;
}

impl Encode<Self> for Pile<'_> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(0);

    type Encode = Self;
    fn encode(self) -> Self {
        self
    }
}

impl EncodePoll for Pile<'_> {
    type Zone = Self;
    type Target = Self;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.done()
    }
}

#[derive(Debug, Clone)]
pub struct Tx<'p> {
    pile: Pile<'p>,
    dst: Vec<u8>,
}

impl<'p> Tx<'p> {
    pub fn new(pile: Pile<'p>) -> Self {
        Self { pile, dst: vec![], }
    }

    pub fn save<T: Save<Pile<'p>>>(&mut self, owned: T::Owned) -> (Offset<'static>, T::Metadata) {
        let mut saver = T::save(owned);
        if let Poll::Ready(Ok(r)) = saver.poll(self) {
            r
        } else {
            panic!()
        }
    }
}

impl<'p> Saver for Tx<'p> {
    type Zone = Pile<'p>;
    type Error = !;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [MaybeUninit<u8>]))
        -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>
    {
        let offset = self.dst.len();
        self.dst.resize(offset + size, 0);
        let dst = &mut self.dst[offset .. offset + size];

        let dst: &mut [MaybeUninit<u8>] = unsafe { mem::transmute(dst) };
        f(dst);

        let offset = unsafe { Offset::new_unchecked(offset) };
        Ok(offset)
    }

    fn save_own<T: ?Sized + Pointee>(&mut self, own: Own<T, Self::Zone>) -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>
        where T: Save<Self::Zone>
    {
        Ok(own.ptr().persist())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let pile = Pile::new(&[]);

        let mut tx = Tx::new(pile);

        dbg!(tx.save::<(u8, bool)>((42u8, true)));
        dbg!(tx.save::<(u8, bool)>((42u8, true)));
    }
}
