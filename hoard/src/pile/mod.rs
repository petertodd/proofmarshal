use std::marker::PhantomData;
use std::fmt;
use std::ptr::NonNull;
use std::alloc::Layout;
use std::mem::ManuallyDrop;

use thiserror::Error;

use crate::blob::{Bytes, BlobDyn, BytesUninit};
use crate::zone::*;
use crate::zone::heap::{Heap, HeapPtr};
use crate::load::{Load, LoadRef, MaybeValid};
use crate::pointee::Pointee;
use crate::bag::Bag;
use crate::owned::{Ref, Take, Own};
use crate::save::{BlobSaver, SaveDirty, SaveDirtyPoll};

pub mod offset;
pub use self::offset::Offset;

pub mod mapping;
use self::mapping::Mapping;

#[derive(Clone, Copy, Debug)]
pub struct PilePtr<'p, 'v> {
    marker: PhantomData<fn(&'p ()) -> (&'p (), &'v ())>,
    offset: Offset,
}

#[derive(Debug)]
pub struct PilePtrMut<'p, 'v, D = HeapPtr> {
    kind: Kind<'p, 'v, D>,
}

#[derive(Debug)]
pub enum Kind<'p, 'v, D> {
    Dirty(D),
    Clean(PilePtr<'p, 'v>),
}

impl From<Offset> for PilePtr<'_, '_> {
    fn from(offset: Offset) -> Self {
        Self {
            marker: PhantomData,
            offset,
        }
    }
}

impl<'p, 'v, D> From<PilePtr<'p, 'v>> for PilePtrMut<'p, 'v, D> {
    fn from(clean: PilePtr<'p, 'v>) -> Self {
        Self::new(Kind::Clean(clean))
    }
}

impl<'p, 'v, D> PilePtrMut<'p, 'v, D> {
    pub fn new(kind: Kind<'p, 'v, D>) -> Self {
        Self {
            kind,
        }
    }

    pub fn kind(&self) -> &Kind<'p, 'v, D> {
        &self.kind
    }

    pub fn kind_mut(&mut self) -> &mut Kind<'p, 'v, D> {
        &mut self.kind
    }
}

impl PtrConst for PilePtr<'_, '_> {
    type Blob = Offset;

    fn to_blob(self) -> Offset {
        self.offset
    }

    fn from_blob(blob: Offset) -> Self {
        blob.into()
    }
}

impl<'p, 'v> FromPtr<Self> for PilePtr<'p, 'v> {
    fn from_ptr(this: Self) -> Self {
        this
    }
}

impl<'p, 'v> AsPtr<Self> for PilePtr<'p, 'v> {
    fn as_ptr(&self) -> &Self {
        self
    }
}

impl<'p, 'v, D> FromPtr<Self> for PilePtrMut<'p, 'v, D> {
    fn from_ptr(this: Self) -> Self {
        this
    }
}

impl<'p, 'v, D> AsPtr<Self> for PilePtrMut<'p, 'v, D> {
    fn as_ptr(&self) -> &Self {
        self
    }
}

impl<'p, 'v, D: Ptr> Ptr for PilePtrMut<'p, 'v, D>
where D::Clean: Into<!>
{
    const NEEDS_DEALLOC: bool = D::NEEDS_DEALLOC;

    type Clean = PilePtr<'p, 'v>;
    type Blob = Offset;

    fn from_clean(clean: PilePtr<'p, 'v>) -> Self {
        clean.into()
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        match &mut self.kind {
            Kind::Clean(_) => {},
            Kind::Dirty(dirty) => {
                dirty.dealloc::<T>(metadata);
            },
        }
    }

    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Clean> {
        match &self.kind {
            Kind::Clean(offset) => Err(*offset),
            Kind::Dirty(dirty) => Ok(dirty.try_get_dirty::<T>(metadata).into_ok()),
        }
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<&mut T, Self::Clean> {
        match &mut self.kind {
            Kind::Clean(offset) => Err(*offset),
            Kind::Dirty(dirty) => Ok(dirty.try_get_dirty_mut::<T>(metadata).into_ok()),
        }
    }

    unsafe fn try_take_dirty_with<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> R
        where F: FnOnce(Result<Own<T>, Self::Clean>) -> R
    {
        match self.kind {
            Kind::Clean(offset) => f(Err(offset)),
            Kind::Dirty(dirty) => {
                dirty.try_take_dirty_with(metadata, |src| {
                    f(Ok(src.into_ok()))
                })
            },
        }
    }

    fn alloc_raw_impl(layout: Layout) -> (NonNull<()>, Self) {
        let (non_null, dirty) = D::alloc_raw_impl(layout);
        (non_null, Self::new(Kind::Dirty(dirty)))
    }
}

impl<D: Default> Default for PilePtrMut<'_, '_, D> {
    fn default() -> Self {
        Self::new(Kind::Dirty(D::default()))
    }
}

#[derive(Debug)]
pub struct Pile<'p, 'v, M: ?Sized> {
    marker: PhantomData<fn(&'p ()) -> (&'p (), &'v [u8])>,
    mapping: &'v M,
}

impl<M: ?Sized> Clone for Pile<'_, '_, M> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            mapping: self.mapping,
        }
    }
}

impl<M: ?Sized> Copy for Pile<'_, '_, M> {}

impl<'p, 'v, M: ?Sized> Pile<'p, 'v, M> {
    pub fn new(mapping: &'v M) -> Self {
        Self {
            marker: PhantomData,
            mapping,
        }
    }
}

impl<'p, 'v, M: ?Sized> AsZone<Pile<'p, 'v, M>> for Pile<'p, 'v, M> {
    fn as_zone(&self) -> &Pile<'p, 'v, M> {
        self
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct Error(Box<dyn std::error::Error + 'static>);

impl Error {
    fn new(err: impl std::error::Error + 'static) -> Self {
        Error(Box::new(err))
    }
}

impl<'p, 'v, M: ?Sized> Zone for Pile<'p, 'v, M> {
    type Error = Error;
    type Ptr = PilePtr<'p, 'v>;
}

impl<'p, 'v, M: ?Sized> Get<PilePtr<'p, 'v>> for Pile<'p, 'v, M>
where M: Mapping,
{
    unsafe fn get_unchecked<'a, T: ?Sized>(
        &'a self,
        ptr: &'a PilePtr<'p, 'v>,
        metadata: T::Metadata
    ) -> Result<MaybeValid<Ref<'a, T>>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>,
    {
        let bytes = self.mapping.deref_bytes::<T::BlobDyn>(ptr.offset, metadata).map_err(Error::new)?;
        T::load_ref_from_bytes(bytes, self.as_zone())
            .map_err(Error::new)
    }

    unsafe fn take_unchecked<T: ?Sized>(
        &self,
        ptr: PilePtr<'p, 'v>,
        metadata: T::Metadata,
    ) -> Result<MaybeValid<T::Owned>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>,
    {
        let bytes = self.mapping.deref_bytes::<T::BlobDyn>(ptr.offset, metadata).map_err(Error::new)?;

        T::load_owned_from_bytes(bytes, self.as_zone())
            .map_err(Error::new)
    }
}

#[derive(Debug)]
pub struct PileMut<'p, 'v, M: ?Sized = [u8], A = Heap> {
    pile: Pile<'p, 'v, M>,
    allocator: A,
}

impl<M: ?Sized, A: Clone> Clone for PileMut<'_, '_, M, A> {
    fn clone(&self) -> Self {
        Self {
            pile: self.pile,
            allocator: self.allocator.clone(),
        }
    }
}
impl<M: ?Sized, A: Copy> Copy for PileMut<'_, '_, M, A> {}

impl<M: ?Sized + Mapping, A: Default> Default for PileMut<'_, 'static, M, A> {
    fn default() -> Self {
        Self {
            pile: Pile::new(M::default_ref()),
            allocator: A::default(),
        }
    }
}

impl<'p, 'v, M: ?Sized + Mapping, A> PileMut<'p, 'v, M, A> {
    pub fn new(mapping: &'v M, allocator: A) -> Self {
        Self {
            pile: Pile::new(mapping),
            allocator,
        }
    }
}

impl<'p, 'v, M: ?Sized, A: Zone> AsZone<PileMut<'p, 'v, M, A>> for PileMut<'p, 'v, M, A> {
    fn as_zone(&self) -> &PileMut<'p, 'v, M, A> {
        self
    }
}

impl<'p, 'v, M: ?Sized, A: Zone> AsZone<Pile<'p, 'v, M>> for PileMut<'p, 'v, M, A> {
    fn as_zone(&self) -> &Pile<'p, 'v, M> {
        &self.pile
    }
}

impl<'p, 'v, M: ?Sized, A> From<PileMut<'p, 'v, M, A>> for Pile<'p, 'v, M> {
    fn from(pile_mut: PileMut<'p, 'v, M, A>) -> Pile<'p, 'v, M> {
        pile_mut.pile
    }
}

impl<'p, 'v, M: ?Sized + Mapping, A: Zone> Zone for PileMut<'p, 'v, M, A>
where <A::Ptr as Ptr>::Clean: Into<!>
{
    type Error = Error;
    type Ptr = PilePtrMut<'p, 'v, A::Ptr>;
}

impl<'p, 'v, M: ?Sized + Mapping, A: Zone> Get<PilePtr<'p, 'v>> for PileMut<'p, 'v, M, A>
where <A::Ptr as Ptr>::Clean: Into<!>
{
    unsafe fn get_unchecked<'a, T: ?Sized>(
        &'a self,
        ptr: &'a PilePtr<'p, 'v>,
        metadata: T::Metadata
    ) -> Result<MaybeValid<Ref<'a, T>>, Self::Error>
        where T: LoadRef,
              Self: AsZone<T::Zone>,
    {
        let bytes = self.pile.mapping.deref_bytes::<T::BlobDyn>(ptr.offset, metadata).map_err(Error::new)?;
        T::load_ref_from_bytes(bytes, self.as_zone())
            .map_err(Error::new)
    }

    unsafe fn take_unchecked<T: ?Sized + LoadRef>(
        &self,
        ptr: PilePtr<'p, 'v>,
        metadata: T::Metadata,
    ) -> Result<MaybeValid<T::Owned>, Self::Error> {
        todo!()
    }
}

impl<'p, 'v, M: ?Sized + Mapping, A: Zone, D: Ptr> Get<PilePtrMut<'p, 'v, D>> for PileMut<'p, 'v, M, A>
where <A::Ptr as Ptr>::Clean: Into<!>,
      D::Clean: Into<!>
{
    unsafe fn get_unchecked<'a, T: ?Sized + LoadRef>(
        &'a self,
        ptr: &'a PilePtrMut<'p, 'v, D>,
        metadata: T::Metadata
    ) -> Result<MaybeValid<Ref<'a, T>>, Self::Error>
        where Self: AsZone<T::Zone>,
    {
        match &ptr.kind {
            Kind::Clean(clean) => {
                let bytes = self.pile.mapping.deref_bytes::<T::BlobDyn>(clean.offset, metadata).map_err(Error::new)?;
                T::load_ref_from_bytes(bytes, self.as_zone())
                    .map_err(Error::new)
            }
            Kind::Dirty(dirty) => {
                let r = dirty.try_get_dirty::<T>(metadata).into_ok();
                Ok(Ref::Borrowed(r).into())
            },
        }
    }

    unsafe fn take_unchecked<T: ?Sized + LoadRef>(
        &self,
        ptr: PilePtrMut<'p, 'v, D>,
        metadata: T::Metadata,
    ) -> Result<MaybeValid<T::Owned>, Self::Error> {
        match ptr.kind {
            Kind::Clean(_clean) => {
                todo!()
            }
            Kind::Dirty(dirty) => {
                let r = dirty.try_take_dirty::<T>(metadata).into_ok();
                Ok(MaybeValid::from(r))
            },
        }
    }
}

impl<'p, 'v, M: ?Sized + Mapping, A: Alloc> GetMut<PilePtrMut<'p, 'v, A::Ptr>> for PileMut<'p, 'v, M, A>
where <A::Ptr as Ptr>::Clean: Into<!>,
{
    unsafe fn get_unchecked_mut<'a, T: ?Sized + LoadRef>(
        &'a self,
        ptr: &'a mut PilePtrMut<'p, 'v, A::Ptr>,
        metadata: T::Metadata
    ) -> Result<MaybeValid<&'a mut T>, Self::Error>
        where Self: AsZone<T::Zone>,
    {
        match &mut ptr.kind {
            Kind::Dirty(dirty) => {
                let r = dirty.try_get_dirty_mut::<T>(metadata).into_ok();
                Ok(r.into())
            },
            Kind::Clean(clean) => {
                todo!()
            },
        }
    }
}

impl<'p, 'v, M: ?Sized + Mapping, A: Alloc> Alloc for PileMut<'p, 'v, M, A>
where <A::Ptr as Ptr>::Clean: Into<!>,
{
    fn alloc_raw(&mut self, layout: core::alloc::Layout) -> (NonNull<()>, Self::Ptr, Self) {
        let (non_null, dirty, allocator) = self.allocator.alloc_raw(layout);
        (non_null,
         PilePtrMut {
             kind: Kind::Dirty(dirty),
         },
         PileMut {
             pile: self.pile,
             allocator,
         })
    }
}

#[derive(Debug)]
pub struct VecSaver<'p, 'v, M: ?Sized> {
    pile: Pile<'p, 'v, M>,
    buf: Vec<u8>,
}

impl<'p, 'v, M: ?Sized> VecSaver<'p, 'v, M>
where M: Mapping + AsRef<[u8]>
{
    pub fn new(pile: Pile<'p, 'v, M>) -> Self {
        Self {
            pile,
            buf: vec![],
        }
    }

    pub fn save_dirty<T: SaveDirty>(mut self, value: &T) -> (Vec<u8>, Offset)
        where T::CleanPtr: FromPtr<PilePtr<'p, 'v>>
    {
        let mut poller = value.init_save_dirty();
        poller.save_dirty_poll(&mut self).into_ok();

        let offset = self.save_bytes((), |dst| {
            poller.encode_blob_bytes(dst)
        }).into_ok();

        (self.buf, offset)
    }
}

impl<'p, 'v, M: ?Sized> BlobSaver for VecSaver<'p, 'v, M>
where M: Mapping + AsRef<[u8]>
{
    type Error = !;
    type CleanPtr = PilePtr<'p, 'v>;

    fn save_bytes<T: ?Sized + BlobDyn>(
        &mut self,
        metadata: T::Metadata,
        f: impl FnOnce(BytesUninit<'_, T>) -> Bytes<'_, T>,
    ) -> Result<Offset, Self::Error>
    {
        let blob_size = T::try_size(metadata).expect("invalid metadata");
        let offset = self.pile.mapping.as_ref().len() + self.buf.len();

        self.buf.resize(offset + blob_size, 0);
        let dst = &mut self.buf[offset .. ];
        let dst = BytesUninit::from_bytes(dst, metadata).unwrap();

        let _ = f(dst);
        Ok(offset.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::error::Error;

    #[test]
    fn test() -> Result<(), Box<dyn Error>> {
        let pile = Pile::new(&[42u8;1][..]);
        let ptr = PilePtr::from(Offset::new(0));

        let bag = unsafe {
            Bag::<u8, _, _>::from_raw_parts(ptr, (), pile)
        };

        let r = bag.get()?;
        assert_eq!(*r, 42);

        Ok(())
    }

    #[test]
    fn test_pilemut_alloc() -> Result<(), Box<dyn Error>> {
        let mut pile = PileMut::<[u8], Heap>::default();

        let bag1 = pile.alloc(42u8);
        let bag2 = pile.alloc(bag1);
        let _ = dbg!(bag2.get());

        Ok(())
    }

    #[test]
    fn saver() {
        let mut pile = PileMut::<[u8], Heap>::default();

        let saver = VecSaver::new(pile.into());
        let (buf, offset) = saver.save_dirty(&42u8);
        assert_eq!(offset, 0u64);
        assert_eq!(buf, &[42]);

        let bag1 = pile.alloc(42u8);

        let saver = VecSaver::new(pile.into());
        let (buf, offset) = saver.save_dirty(&bag1);
        assert_eq!(offset, 1u64);
        assert_eq!(buf, &[42, 0,0,0,0,0,0,0,0]);

        let bag2 = pile.alloc(bag1);

        let saver = VecSaver::new(pile.into());
        let (buf, offset) = saver.save_dirty(&bag2);
        assert_eq!(offset, 9u64);
        assert_eq!(buf, &[42,
                           0,0,0,0,0,0,0,0,
                           1,0,0,0,0,0,0,0]);
    }
}
