use super::*;

use std::marker::PhantomData;
use std::mem;
use std::task::Poll;

mod scalars;
mod tuples;

mod blob;
pub use self::blob::Blob;

pub trait Marshal<Z: Zone> : Clone {
    type Error : fmt::Debug;

    fn pile_layout() -> pile::Layout
        where Z: pile::Pile;

    fn pile_load<'p>(blob: Blob<'p, Self, Z>, pile: &Z) -> Result<Cow<'p, Self>, Self::Error>
        where Z: pile::Pile;

    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile;
}

impl<Z: Zone, T: Marshal<Z>> Load<Z> for T {
    type Error = T::Error;

    #[inline]
    fn pile_load<'p, L>(pile: &Z, rec: &'p Rec<Self, Z>) -> Result<Result<Cow<'p, Self>, Self::Error>, Z::Error>
        where Z: pile::Pile
    {
        let blob = pile.get_blob(&rec.ptr().raw, T::pile_layout().size())?;
        Ok(T::pile_load(Blob::new(blob), pile))
    }
}

impl<Z: Zone, T: Marshal<Z>> Store<Z> for T {
    #[inline]
    unsafe fn alloc(owned: T, dst: *mut ()) -> *mut Self {
        let dst = dst.cast::<T>();
        dst.cast::<T>().write(owned);
        dst
    }

    #[inline]
    fn pile_store<D: pile::Dumper<Pile=Z>>(owned: T, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        owned.pile_store(dumper)
    }
}


#[derive(Debug)]
pub struct LoadRecError;

impl<T: ?Sized + Store<Z>, Z: Zone> Marshal<Z> for Rec<T,Z>
where T::Metadata: Marshal<Z>,
{
    type Error = LoadRecError;

    #[inline(always)]
    fn pile_layout() -> pile::Layout
        where Z: pile::Pile
    {
        Z::OFFSET_LAYOUT.extend(T::Metadata::pile_layout())
    }

    #[inline(always)]
    fn pile_load<'p>(blob: Blob<'p, Self, Z>, pile: &Z) -> Result<Cow<'p, Self>, Self::Error>
        where Z: pile::Pile
    {
        todo!()
    }

    #[inline(always)]
    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        let (dumper, offset) = dumper.dump_rec(self)?;

        let dst = vec![0; Self::pile_layout().size()];

        StructDumper::new(dumper, dst)
                     .dump_value(&offset)?
                     .dump_value(&self.ptr.metadata)?
                     .done()
    }
}

#[derive(Debug)]
pub struct StructDumper<D, B> {
    dumper: D,
    dst: B,
    written: usize,
}

impl<D, B> StructDumper<D, B> {
    #[inline]
    pub fn new(dumper: D, dst: B) -> Self {
        Self {
            dumper, dst,
            written: 0,
        }
    }
}

impl<D: pile::Dumper, B: AsMut<[u8]>> StructDumper<D, B> {
    #[inline]
    pub fn dump_value<T: Marshal<D::Pile>>(mut self, value: &T) -> Result<Self, D::Error> {
        let value_size = T::pile_layout().size();
        assert!(self.written + value_size <= self.dst.as_mut().len(),
                "overflow");

        let (_, remaining) = self.dst.as_mut().split_at_mut(self.written);
        let (field_dst, _) = remaining.split_at_mut(value_size);

        let field_dumper = value.pile_store(FieldDumper::new(self.dumper, field_dst))?;
        Ok(Self {
            dumper: field_dumper.dumper,
            dst: self.dst,
            written: self.written + value_size,
        })
    }

    #[inline]
    pub fn done(mut self) -> Result<D::Done, D::Error> {
        assert_eq!(self.written, self.dst.as_mut().len(),
                   "not all bytes written");
        self.dumper.dump_blob(self.dst.as_mut())
    }
}

#[derive(Debug)]
struct FieldDumper<'a, D> {
    dumper: D,
    dst: &'a mut [u8],
}

impl<'a, D> FieldDumper<'a, D> {
    fn new(dumper: D, dst: &'a mut [u8]) -> Self {
        Self { dumper, dst }
    }
}

impl<D: pile::Dumper> pile::Dumper for FieldDumper<'_, D> {
    type Pile = D::Pile;
    type Error = D::Error;
    type Done = Self;

    #[inline(always)]
    fn dump_rec<T: ?Sized + Store<Self::Pile>>(self, rec: &Rec<T, Self::Pile>)
        -> Result<(Self, <Self::Pile as pile::Pile>::Offset), Self::Error>
    {
        let (dumper, offset) = self.dumper.dump_rec(rec)?;

        Ok((Self::new(dumper, self.dst), offset))
    }

    #[inline(always)]
    fn dump_blob(self, buf: &[u8]) -> Result<Self, Self::Error> {
        assert_eq!(buf.len(), self.dst.len());
        self.dst.copy_from_slice(buf);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SimplePile<'a>(&'a [u8]);

    impl Zone for SimplePile<'_> {
        type Ptr = u64;
        type Allocator = crate::never::NeverAlloc<Self>;
        type Error = !;

        fn clone_rec<T: ?Sized + Pointee>(r: &Rec<T,Self>) -> Rec<T,Self> {
            todo!()
        }

        unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T,Self>) {
            let _ = ptr.raw;
        }
    }

    impl pile::Pile for SimplePile<'_> {
        const OFFSET_LAYOUT: pile::Layout = pile::Layout::new(8);
        type Offset = u64;

        fn get_blob<'p>(&self, ptr: &'p Self::Ptr, size: usize) -> Result<&'p [u8], Self::Error> {
            todo!()
        }
    }

    #[derive(Debug)]
    struct SimpleDumper<'a> {
        pile: SimplePile<'a>,
        dst: Vec<u8>,
    }

    impl<'a> pile::Dumper for SimpleDumper<'a> {
        type Pile = SimplePile<'a>;
        type Error = !;
        type Done = (Vec<u8>, u64);

        fn dump_rec<T: ?Sized + Store<Self::Pile>>(self, rec: &Rec<T, Self::Pile>) -> Result<(Self, u64), Self::Error> {
            Ok((self, rec.ptr().raw))
        }

        fn dump_blob(mut self, buf: &[u8]) -> Result<Self::Done, Self::Error> {
            let offset = self.dst.len() as u64;
            self.dst.extend_from_slice(buf);
            Ok((self.dst, offset))
        }
    }

    #[test]
    fn test() {
        let buf = &[];
        let pile = SimplePile(buf);

        let ptr: Ptr<u8, SimplePile> = Ptr { raw: 0x1122_3344_5566_7788, metadata: () };
        let rec = unsafe { Rec::from_ptr(ptr) };

        let dumper = SimpleDumper { pile, dst: vec![] };

        let r = (128u8, rec).pile_store(dumper).unwrap();

        dbg!(r);
    }
}
