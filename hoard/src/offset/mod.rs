use std::marker::PhantomData;
use std::convert::TryFrom;
use std::fmt;

use thiserror::Error;

use crate::blob::{Blob, BlobDyn, Bytes, BytesUninit};
use crate::primitive::Primitive;
use crate::ptr::{self, Ptr, PtrClean, PtrBlob, TryGet, TryGetMut, Get, GetMut};
use crate::owned::{Ref, RefOwn, IntoOwned, Take};
use crate::pointee::Pointee;
use crate::load::{LoadRefIn, MaybeValid};
use crate::bag::Bag;

#[derive(Clone, Copy, Debug)]
pub struct Offset<M = ()> {
    offset: u64,
    mapping: M,
}

#[derive(Debug)]
pub enum OffsetMut<M> {
    Heap(ptr::Heap),
    Offset(Offset<M>),
}

impl<M> Offset<M> {
    pub fn new(offset: u64, mapping: M) -> Self {
        Self { offset, mapping }
    }

    pub fn to_blob(&self) -> Offset {
        Offset::new(self.offset, ())
    }
}

impl<M> From<Offset<M>> for OffsetMut<M> {
    fn from(offset: Offset<M>) -> Self {
        Self::Offset(offset)
    }
}

impl Primitive for Offset {
    const BLOB_SIZE: usize = 8;
    type DecodeBytesError = !;

    fn decode_blob_bytes(blob: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let buf = TryFrom::try_from(&blob[..]).unwrap();
        Ok(Self::new(u64::from_le_bytes(buf), ()))
    }

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        /*
        dst.write_bytes(&self.0.to_le_bytes())
        */ todo!()
    }
}

impl<M> From<!> for Offset<M> {
    fn from(never: !) -> Self {
        match never {}
    }
}

impl PtrBlob for Offset {
}

impl<'m, M: ?Sized> PtrClean for Offset<&'m M> {
    type Blob = Offset;
    type Zone = &'m M;

    fn zone(&self) -> Self::Zone {
        self.mapping
    }

    fn to_blob(self) -> Self::Blob {
        Offset::new(self.offset, ())
    }

    fn from_blob(blob: Self::Blob, zone: &Self::Zone) -> Self {
        Self::new(blob.offset, *zone)
    }
}

impl<'m, M: ?Sized> Ptr for OffsetMut<&'m M>
{
    type Zone = &'m M;
    type Clean = Offset<&'m M>;
    type Blob = Offset;

    fn from_clean(clean: Self::Clean) -> Self {
        Self::Offset(clean)
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        match self {
            Self::Offset(_) => {},
            Self::Heap(heap) => heap.dealloc::<T>(metadata),
        }
    }

    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<MaybeValid<&T>, Self::Clean> {
        match self {
            Self::Offset(offset) => Err(*offset),
            Self::Heap(heap) => heap.try_get_dirty::<T>(metadata)
                                    .map_err(|never| match never {})
        }
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Clean> {
        match self {
            Self::Offset(offset) => Err(*offset),
            Self::Heap(heap) => Ok(heap.try_get_dirty_mut::<T>(metadata).into_ok())
        }
    }

    unsafe fn try_take_dirty_then<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Clean>
        where F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        match self {
            Self::Offset(offset) => Err(offset),
            Self::Heap(heap) => Ok(heap.try_take_dirty_then::<T, _, _>(metadata, f).into_ok()),
        }
    }
}

pub trait Mapping {
    type Error : 'static + std::error::Error + Send;

    fn try_get_blob<F, R>(&self, offset: Offset, len: usize, f: F) -> R
        where F: FnOnce(Result<&[u8], Self::Error>) -> R;

    #[track_caller]
    fn handle_error(&self, err: Error<Self::Error>) -> ! {
        panic!("get failed: {:?}", err)
    }
}


impl<T: ?Sized + Mapping> Mapping for &'_ T {
    type Error = T::Error;

    fn try_get_blob<F, R>(&self, offset: Offset, len: usize, f: F) -> R
        where F: FnOnce(Result<&[u8], Self::Error>) -> R
    {
        (**self).try_get_blob(offset, len, f)
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
#[non_exhaustive]
pub struct SliceGetBlobError;

impl Mapping for [u8] {
    type Error = SliceGetBlobError;

    fn try_get_blob<F, R>(&self, offset: Offset, len: usize, f: F) -> R
        where F: FnOnce(Result<&[u8], Self::Error>) -> R
    {
        let maybe_bytes = usize::try_from(offset.offset).ok().and_then(|start| {
            start.checked_add(len).and_then(|end| Some(start .. end))
        }).and_then(|range| self.get(range))
          .ok_or(SliceGetBlobError);

        f(maybe_bytes)
    }
}


#[derive(Debug, Error)]
#[error("FIXME")]
pub struct Error<E: std::error::Error> {
    inner: Box<Inner<E>>,
}

#[derive(Debug)]
struct Inner<E> {
    offset: Offset,
    kind: ErrorKind<E>,
}

#[derive(Debug)]
pub enum ErrorKind<E> {
    GetBlob(E),
    Decode(Box<dyn std::error::Error + 'static + Send>),
}

impl<E: std::error::Error> Error<E> {
    fn new<T: ?Sized>(offset: Offset, metadata: T::Metadata, kind: ErrorKind<E>) -> Self
        where T: BlobDyn
    {
        Self {
            inner: Box::new(Inner {
                offset,
                kind,
            })
        }
    }
}

impl<'m, M: ?Sized> TryGet for Offset<&'m M>
where M: Mapping
{
    type Error = Error<M::Error>;

    #[track_caller]
    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: LoadRefIn<&'m M>
    {
        let len = T::BlobDyn::try_size(metadata).expect("valid metadata");
        self.mapping.try_get_blob(self.to_blob(), len, |maybe_blob| {
            match maybe_blob {
                Ok(bytes) => {
                    let bytes = Bytes::<T::BlobDyn>::try_from_slice(bytes, metadata).expect("correct slice");

                    match T::load_owned_from_bytes_in(bytes, &self.mapping) {
                        Ok(maybe_valid_owned) => {
                            let owned = maybe_valid_owned.trust();
                            Ok(MaybeValid::from(Ref::Owned(owned)))
                        },
                        Err(err) => {
                            Err(Error::new::<T::BlobDyn>(
                                    self.to_blob(),
                                    metadata,
                                    ErrorKind::Decode(Box::new(err))))
                        },
                    }
                },
                Err(err) => {
                    Err(Error::new::<T::BlobDyn>(self.to_blob(), metadata, ErrorKind::GetBlob(err)))
                },
            }
        })
    }

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        match self.try_get::<T>(metadata)?.trust() {
            Ref::Owned(owned) => {
                Ok(owned.take_unsized(|r: RefOwn<T>| f(MaybeValid::new(r))))
            },
            Ref::Borrowed(_borrowed) => unreachable!(),
        }
    }

    unsafe fn try_take<T: ?Sized>(self, metadata: T::Metadata) -> Result<MaybeValid<T::Owned>, Self::Error>
        where T: LoadRefIn<Self::Zone>,
    {
        match self.try_get::<T>(metadata)?.trust() {
            Ref::Owned(owned) => Ok(MaybeValid::new(owned)),
            Ref::Borrowed(_) => unreachable!(),
        }
    }
}

impl<'m, M: ?Sized> TryGet for OffsetMut<&'m M>
where M: Mapping
{
    type Error = Error<M::Error>;

    #[track_caller]
    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: LoadRefIn<&'m M>
    {
        match &self {
            Self::Offset(offset) => offset.try_get::<T>(metadata),
            Self::Heap(ptr) => ptr.try_get_dirty::<T>(metadata)
                                  .map(|r| MaybeValid::from(Ref::Borrowed(r.trust())))
                                  .map_err(|never| match never {})
        }
    }

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        match self {
            Self::Offset(offset) => offset.try_take_then(metadata, f),
            Self::Heap(ptr) => Ok(ptr.try_take_dirty_then(metadata, f).into_ok()),
        }
    }

    unsafe fn try_take<T: ?Sized>(self, metadata: T::Metadata) -> Result<MaybeValid<T::Owned>, Self::Error>
        where T: LoadRefIn<Self::Zone>,
    {
        match self {
            Self::Offset(offset) => offset.try_take::<T>(metadata),
            Self::Heap(ptr) => Ok(ptr.try_take_dirty::<T>(metadata).into_ok()),
        }
    }
}

impl<'m, M: ?Sized> Get for Offset<&'m M>
where M: Mapping
{
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: LoadRefIn<&'m M>
    {
        match self.try_get(metadata) {
            Ok(r) => r,
            Err(err) => self.mapping.handle_error(err),
        }
    }

    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        let mapping = self.mapping;
        match self.try_take_then(metadata, f) {
            Ok(r) => r,
            Err(err) => self.mapping.handle_error(err),
        }
    }

    unsafe fn take<T: ?Sized>(self, metadata: T::Metadata) -> MaybeValid<T::Owned>
        where T: LoadRefIn<Self::Zone>,
    {
        let mapping = self.mapping;
        match self.try_take::<T>(metadata) {
            Ok(r) => r,
            Err(err) => self.mapping.handle_error(err),
        }
    }
}

impl<'m, M: ?Sized> Get for OffsetMut<&'m M>
where M: Mapping
{
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: LoadRefIn<&'m M>
    {
        match self {
            Self::Offset(offset) => offset.get::<T>(metadata),
            Self::Heap(ptr) => {
                let r = ptr.try_get_dirty::<T>(metadata).into_ok();
                MaybeValid::new(Ref::Borrowed(r.trust()))
            }
        }
    }

    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        match self {
            Self::Offset(offset) => offset.take_then(metadata, f),
            Self::Heap(ptr) => ptr.try_take_dirty_then(metadata, f).into_ok(),
        }
    }

    unsafe fn take<T: ?Sized>(self, metadata: T::Metadata) -> MaybeValid<T::Owned>
        where T: LoadRefIn<Self::Zone>,
    {
        match self {
            Self::Offset(offset) => offset.take::<T>(metadata),
            Self::Heap(ptr) => ptr.try_take_dirty::<T>(metadata).into_ok(),
        }
    }
}

impl<'m, M: ?Sized> OffsetMut<&'m M>
where M: Mapping
{
    unsafe fn try_make_dirty<T: ?Sized>(&mut self, offset: Offset<&'m M>, metadata: T::Metadata)
        -> Result<MaybeValid<&mut T>, Error<M::Error>>
            where T: LoadRefIn<&'m M>
    {
        let owned = offset.try_take::<T>(metadata)?.trust();
        let bag: Bag<T, ptr::Heap> = ptr::Heap::alloc(owned);

        let (ptr, metadata2) = bag.into_raw_parts();
        assert_eq!(metadata, metadata2);
        *self = Self::Heap(ptr);
        self.try_get_mut::<T>(metadata)
    }
}

impl<'m, M: ?Sized> TryGetMut for OffsetMut<&'m M>
where M: Mapping
{
    unsafe fn try_get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Error>
        where T: LoadRefIn<&'m M>
    {
        match *self {
            Self::Heap(ref mut ptr) => {
                Ok(ptr.try_get_dirty_mut::<T>(metadata).into_ok().into())
            },
            Self::Offset(offset) => self.try_make_dirty(offset, metadata),
        }
    }
}

impl<'m, M: ?Sized> OffsetMut<&'m M>
where M: Mapping
{
    unsafe fn make_dirty<T: ?Sized>(&mut self, offset: Offset<&'m M>, metadata: T::Metadata)
        -> MaybeValid<&mut T>
            where T: LoadRefIn<&'m M>
    {
        let owned = offset.take::<T>(metadata).trust();
        let bag: Bag<T, ptr::Heap> = ptr::Heap::alloc(owned);

        let (ptr, metadata2) = bag.into_raw_parts();
        assert_eq!(metadata, metadata2);
        *self = Self::Heap(ptr);
        self.get_mut::<T>(metadata)
    }
}

impl<'m, M: ?Sized> GetMut for OffsetMut<&'m M>
where M: Mapping
{
    unsafe fn get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> MaybeValid<&mut T>
        where T: LoadRefIn<&'m M>
    {
        match *self {
            Self::Heap(ref mut ptr) => {
                ptr.try_get_dirty_mut::<T>(metadata).into_ok().into()
            },
            Self::Offset(offset) => self.make_dirty(offset, metadata),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bag::Bag;

    #[test]
    fn offset_try_get() {
        let offset = Offset::new(0, &[1u8, 2u8, 3u8, 4u8][..]);

        let bag: Bag<u32, Offset<&[u8]>> = unsafe { Bag::from_raw_parts(offset, ()) };
        assert_eq!(bag.try_get().unwrap(),
                   &0x04030201);
    }

    #[test]
    fn offsetmut_try_get_mut() {
        let offset = Offset::new(0, &[0x12u8, 0x34u8, 0x56u8, 0x78u8][..]);
        let offsetmut = OffsetMut::from(offset);

        let mut bag: Bag<u32, OffsetMut<&[u8]>> = unsafe { Bag::from_raw_parts(offsetmut, ()) };
        assert_eq!(bag.try_get().unwrap(),
                   &0x78563412);

        assert!(matches!(bag.ptr(), OffsetMut::Offset(offset)));

        let r_n = bag.try_get_mut().unwrap();
        assert_eq!(*r_n, 0x78563412);
        assert!(matches!(bag.ptr(), OffsetMut::Heap(_)));

        let r_n = bag.try_get_mut().unwrap();
        *r_n = 0x12345678;

        let r_n = bag.try_get_mut().unwrap();
        assert_eq!(*r_n, 0x12345678);

        dbg!(&bag);
    }

    #[test]
    #[should_panic]
    fn offsetmut_get_error_handling() {
        let offset = Offset::new(0, &[][..]);
        let offsetmut = OffsetMut::from(offset);

        let mut bag: Bag<u32, OffsetMut<&[u8]>> = unsafe { Bag::from_raw_parts(offsetmut, ()) };
        assert_eq!(bag.get(), &0x78563412);
    }
}
