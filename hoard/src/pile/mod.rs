//! Persistence via "piles" of copy-on-write append-only bytes.
//!
//! A `Pile` is a memory zone that (conceptually) consists of a linear byte slice. Pointers to data
//! within a pile are simply 64-bit, little-endian, integer `Offset`'s from the beginning of the
//! slice. The byte slice can come from either volatile memory (eg a `Vec<u8>`) or be a
//! memory-mapped file. `Offset` implements `Persist`, allowing types containing `Offset` pointers
//! to be memory-mapped.
//!
//! Mutation is provided by `PileMut` and `OffsetMut`, which extend `Offset` with copy-on-write
//! semantics: an `OffsetMut` is either a simple `Offset`, or a pointer to heap-allocated memory.
//! `OffsetMut` pointers also implement `Persist`, using the least-significant-bit to distinguish
//! between persistant offsets and heap memory pointers.

use std::marker::PhantomData;
use std::borrow::Borrow;
use std::mem::ManuallyDrop;
use std::ops::Deref;

use thiserror::Error;
use singlelife::Unique;

use owned::Take;

use crate::pointee::Pointee;
use crate::ptr::*;
use crate::offset::*;
use crate::refs::Ref;
use crate::load::*;
use crate::blob::*;

#[derive(Debug, Clone, Copy)]
pub struct TryPile<'p, 'v> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    buf: &'v [u8],
}

impl<'p> Default for TryPile<'p, 'static> {
    fn default() -> Self {
        unsafe { Self::new_unchecked(&[]) }
    }
}

impl<'p, 'v> TryPile<'p, 'v> {
    pub unsafe fn new_unchecked(buf: &'v [u8]) -> Self {
        Self { marker: PhantomData, buf, }
    }
}

pub struct TryPilePtr<'p, 'v> {
    offset: Offset<'p, 'v>,
    pile: TryPile<'p, 'v>,
}

unsafe impl<'p, 'v> ValidateBlob for TryPilePtr<'p, 'v> {
    type BlobError = <Offset<'p, 'v> as ValidateBlob>::BlobError;

    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(<Offset<'p, 'v> as ValidateBlob>::blob_layout())
    }

    fn validate_blob(blob: Blob<Self>, ignore_padding: bool) -> Result<ValidBlob<Self>, Self::BlobError> {
        let mut fields = blob.validate_fields(ignore_padding);
        fields.validate_blob::<Offset<'p, 'v>>()?;
        unsafe { Ok(fields.finish()) }
    }
}

impl<'p, 'v> Load for TryPilePtr<'p, 'v> {
    type Ptr = Self;

    fn decode_blob(blob: ValidBlob<Self>, pile: &TryPile<'p, 'v>) -> Self {
        let mut fields = blob.decode_fields(pile);
        let offset = unsafe { fields.decode_unchecked() };
        fields.finish();

        Self {
            pile: *pile,
            offset,
        }
    }
}

impl AsZone<Self> for TryPile<'_, '_> {
    fn as_zone(&self) -> &Self {
        self
    }
}

impl AsZone<()> for TryPile<'_, '_> {
    fn as_zone(&self) -> &() {
        &()
    }
}

impl AsPtrImpl<Self> for TryPilePtr<'_, '_> {
    fn as_ptr_impl(this: &Self) -> &Self {
        this
    }
}

impl<'p, 'v> Ptr for TryPilePtr<'p, 'v> {
    type Zone = TryPile<'p, 'v>;
    type BlobZone = TryPile<'p, 'v>;
    type Persist = Offset<'p, 'v>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        self.offset.dealloc::<T>(metadata)
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        self.offset.try_get_dirty_unchecked::<T>(metadata)
    }
}

pub struct TryPilePtrMut<'p, 'v> {
    offset: OffsetMut<'p, 'v>,
    pile: TryPile<'p, 'v>,
}

unsafe impl<'p, 'v> ValidateBlob for TryPilePtrMut<'p, 'v> {
    type BlobError = <Offset<'p, 'v> as ValidateBlob>::BlobError;

    #[inline]
    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(<Offset<'p, 'v> as ValidateBlob>::blob_layout())
    }

    fn validate_blob(blob: Blob<Self>, ignore_padding: bool) -> Result<ValidBlob<Self>, Self::BlobError> {
        let mut fields = blob.validate_fields(ignore_padding);
        fields.validate_blob::<Offset<'p, 'v>>()?;
        unsafe { Ok(fields.finish()) }
    }
}

impl<'p, 'v> Load for TryPilePtrMut<'p, 'v> {
    type Ptr = Self;

    fn decode_blob(blob: ValidBlob<Self>, pile: &TryPile<'p, 'v>) -> Self {
        let mut fields = blob.decode_fields(pile);
        let offset = unsafe { fields.decode_unchecked() };
        fields.finish();

        Self {
            pile: *pile,
            offset,
        }
    }
}

impl AsPtrImpl<Self> for TryPilePtrMut<'_, '_> {
    fn as_ptr_impl(this: &Self) -> &Self {
        this
    }
}

impl<'p, 'v> Ptr for TryPilePtrMut<'p, 'v> {
    type Zone = TryPile<'p, 'v>;
    type BlobZone = TryPile<'p, 'v>;
    type Persist = Offset<'p, 'v>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        self.offset.dealloc::<T>(metadata)
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        self.offset.try_get_dirty_unchecked::<T>(metadata)
    }
}

/*
impl<'p, 'v> Ptr for TryPilePtr<'p, 'v> {
    type Zone = TryPile<'p, 'v>;
    type BlobZone = TryPile<'p, 'v>;
    type Persist = Offset<'p, 'v>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, _: T::Metadata) -> Result<&T, Self::Persist> {
        Err(self.offset)
    }
}
*/

/*
    pub fn get_blob<T: ?Sized>(&self, offset: Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Blob<'v, T>, GetBlobError<T::LayoutError>>
        where T: BlobLen
    {
        let blob_len = T::try_blob_len(metadata)
                         .map_err(GetBlobError::Layout)?;

        let start = offset.get();
        start.checked_add(blob_len)
             .and_then(|end| self.buf.get(start .. end))
             .map(|slice| unsafe { Blob::new_unchecked(slice, metadata) })
             .ok_or(GetBlobError::OutOfRange)
    }

    pub fn get_valid_blob<T: ?Sized>(&self, offset: Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<ValidBlob<'v, T>, GetValidBlobError<T::LayoutError, T::Error>>
        where T: ValidateBlobPtr
    {
        let blob = self.get_blob::<T>(offset, metadata)?;

        T::validate_blob_ptr(blob.into())
          .map_err(GetValidBlobError::Validate)
    }

    pub unsafe fn extend_unchecked<'v2>(&self, new_buf: &'v2 [u8]) -> TryPile<'p, 'v2>
        where 'v: 'v2
    {
        debug_assert!(new_buf.starts_with(self.buf));
        TryPile::new_unchecked(new_buf)
    }

    pub fn save_to_vec<T: ?Sized>(&self, tip: &T) -> (Vec<u8>, Offset<'p, 'v>)
        where T: Save<OffsetMut<'p, 'v>, Offset<'p, 'v>>
    {
        ShallowDumper::from_buf(self.buf)
                      .save(tip)
    }

    fn coerce_valid(&self) -> &Pile<'p, 'v> {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<'p, 'v> TryGet<Offset<'p, 'v>> for TryPile<'p, 'v> {
    type Error = Box<dyn std::error::Error>;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<Offset<'p, 'v>>
    {
        let blob = self.get_valid_blob::<T>(*ptr, metadata)?;

        let loader = BlobDecoder::new(blob, self.coerce_valid());
        Ok(T::deref_blob(loader))
    }

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<Offset<'p, 'v>>
    {
        let blob = self.get_valid_blob::<T>(ptr, metadata)?;
        let loader = BlobDecoder::new(blob, self.coerce_valid());
        Ok(T::load_blob(loader))
    }
}

impl<'p, 'v> TryGet<OffsetMut<'p, 'v>> for TryPile<'p, 'v> {
    type Error = !;

    unsafe fn try_get_unchecked<'a, T: ?Sized>(&self, ptr: &'a OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T>, Self::Error>
        where T: Load<OffsetMut<'p, 'v>>
    {
        match ptr.try_get_dirty_unchecked::<T>(metadata) {
            Ok(r) => Ok(Ref::Ref(r)),
            Err(offset) => {
                let blob = self.get_valid_blob::<T>(offset.cast(), metadata)
                               .ok().unwrap();

                let loader = BlobDecoder::new(blob, self.coerce_valid());
                Ok(T::deref_blob(loader))
            }
        }
    }

    unsafe fn try_take_unchecked<'a, T: ?Sized>(&self, ptr: OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<T::Owned, Self::Error>
        where T: Load<OffsetMut<'p, 'v>>
    {
        match ptr.try_take_dirty_unchecked::<T>(metadata) {
            Ok(owned) => Ok(owned),
            Err(offset) => {
                let blob = self.get_valid_blob::<T>(offset.cast(), metadata)
                               .ok().unwrap();

                let loader = BlobDecoder::new(blob, self.coerce_valid());
                Ok(T::load_blob(loader))
            }
        }
    }
}

impl<'p, 'v> Alloc for TryPile<'p, 'v> {
    type Zone = Self;
    type Ptr = OffsetMut<'p, 'v>;

    fn zone(&self) -> Self {
        *self
    }

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr> {
        OffsetMut::alloc(src)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pile<'p, 'v>(TryPile<'p, 'v>);

impl<'p, 'v> AsZone<Self> for Pile<'p, 'v> {
    fn as_zone(&self) -> &Self {
        self
    }
}

impl<'p, 'v> AsZone<TryPile<'p, 'v>> for Pile<'p, 'v> {
    fn as_zone(&self) -> &TryPile<'p, 'v> {
        &self.0
    }
}

impl<'p, 'v> Deref for Pile<'p, 'v> {
    type Target = TryPile<'p, 'v>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'p, 'v> AsRef<TryPile<'p, 'v>> for Pile<'p, 'v> {
    fn as_ref(&self) -> &TryPile<'p, 'v> {
        &self.0
    }
}
impl<'p, 'v> Borrow<TryPile<'p, 'v>> for Pile<'p, 'v> {
    fn borrow(&self) -> &TryPile<'p, 'v> {
        &self.0
    }
}
impl<'p, 'v> From<Pile<'p, 'v>> for TryPile<'p, 'v> {
    fn from(pile: Pile<'p, 'v>) -> Self {
        pile.0
    }
}
impl<'p> Default for Pile<'p, 'static> {
    fn default() -> Self {
        Self(TryPile::default())
    }
}

impl<'p, 'v> Pile<'p, 'v> {
    pub unsafe fn new_unchecked(buf: &'v [u8]) -> Self {
        Self(TryPile::new_unchecked(buf))
    }

    pub unsafe fn extend_unchecked<'v2>(&self, new_buf: &'v2 [u8]) -> Pile<'p, 'v2>
        where 'v: 'v2
    {
        Pile(self.0.extend_unchecked(new_buf))
    }
}

impl<'p, 'v> Get<Offset<'p, 'v>> for Pile<'p, 'v> {
    unsafe fn get_unchecked<'a, T: ?Sized>(&self, ptr: &'a Offset<'p, 'v>, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<Offset<'p, 'v>>
    {
        let blob = self.get_valid_blob::<T>(*ptr, metadata)
                       .unwrap();

        let loader = BlobDecoder::new(blob, self);
        T::deref_blob(loader)
    }

    unsafe fn take_unchecked<'a, T: ?Sized>(&self, ptr: Offset<'p, 'v>, metadata: T::Metadata) -> T::Owned
        where T: Load<Offset<'p, 'v>>
    {
        let blob = self.get_valid_blob::<T>(ptr, metadata)
                       .unwrap();
        let loader = BlobDecoder::new(blob, self);
        T::load_blob(loader)
    }
}

impl<'p, 'v> Get<OffsetMut<'p, 'v>> for Pile<'p, 'v> {
    unsafe fn get_unchecked<'a, T: ?Sized>(&self, ptr: &'a OffsetMut<'p, 'v>, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<OffsetMut<'p, 'v>>
    {
        match ptr.try_get_dirty_unchecked::<T>(metadata) {
            Ok(r) => Ref::Ref(r),
            Err(offset) => {
                let blob = self.get_valid_blob::<T>(offset.cast(), metadata)
                               .unwrap();

                let loader = BlobDecoder::new(blob, self);
                T::deref_blob(loader)
            }
        }
    }

    unsafe fn take_unchecked<'a, T: ?Sized>(&self, ptr: OffsetMut<'p, 'v>, metadata: T::Metadata) -> T::Owned
        where T: Load<OffsetMut<'p, 'v>>
    {
        match ptr.try_take_dirty_unchecked::<T>(metadata) {
            Ok(owned) => owned,
            Err(offset) => {
                let blob = self.get_valid_blob::<T>(offset.cast(), metadata)
                               .unwrap();

                let loader = BlobDecoder::new(blob, self);
                T::load_blob(loader)
            }
        }
    }
}

impl<'p, 'v> GetMut<OffsetMut<'p, 'v>> for Pile<'p, 'v> {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut OffsetMut<'p, 'v>, metadata: T::Metadata) -> &'a mut T
        where T: Load<OffsetMut<'p, 'v>>
    {
        match ptr.kind() {
            Kind::Ptr(ptr) => {
                &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata)
            },
            Kind::Offset(offset) => {
                let blob = self.get_valid_blob::<T>(offset.cast(), metadata)
                               .unwrap();

                let loader = BlobDecoder::new(blob, self);
                let owned: T::Owned = T::load_blob(loader);

                let fat = OffsetMut::alloc(owned).into_inner();
                *ptr = fat.raw;

                self.get_mut_unchecked::<T>(ptr, metadata)
            },
        }
    }
}

impl<'p, 'v> Alloc for Pile<'p, 'v> {
    type Zone = Self;
    type Ptr = OffsetMut<'p, 'v>;

    fn zone(&self) -> Self {
        *self
    }

    fn alloc_own<T: ?Sized + Pointee, U: Take<T>>(&mut self, src: U) -> Own<T, Self::Ptr> {
        OffsetMut::alloc(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bag::Bag;

    #[test]
    fn test() {
        let pile = TryPile::default();
        let offset = Offset::new(0).unwrap();

        unsafe {
            let _: () = pile.try_take_unchecked::<()>(offset, ()).unwrap();
            let _ = pile.try_take_unchecked::<u8>(offset, ()).unwrap_err();
        }
    }

    #[test]
    fn test_alloc() {
        let pile = TryPile::default();
        let bag = Bag::new_in(42u8, pile);
        let bag2 = Bag::new_in(bag, pile);
    }

    #[test]
    fn test_pile() {
        let pile = Pile::default();
        let bag = Bag::new_in(42u8, pile);
        bag.get();
    }

    #[test]
    fn test_recursive_get() {
        let pile = Pile::default();
        let bag1 = Bag::new_in(1u8, pile);
        bag1.get();
        let bag2 = Bag::new_in(bag1, pile);
        bag2.get();
    }

    #[test]
    fn test_extend() {
        let pile1 = Pile::default();
        let bag1 = Bag::new_in(1u8, pile1);
        bag1.get();

        let buf2 = vec![1,2,3];
        let pile2 = unsafe { pile1.extend_unchecked(&buf2) };
        let bag2 = Bag::new_in(bag1, pile2);
        bag2.get();

        assert_eq!(bag2.take().take(), 1);
    }
}

/*
pub mod mapping;
use self::mapping::Mapping;

pub mod error;
use self::error::{Error, ErrorKind};

trait PileZone<'p, 'v> {
    fn get_try_pile(&self) -> TryPile<'p, 'v>;

    fn mapping(&self) -> &'p dyn Mapping;

    fn slice(&self) -> &'p &'p [u8] {
        unsafe {
            &*(self.mapping() as *const dyn Mapping as *const &'p [u8])
        }
    }
}



impl ValidateBlob for TryPile<'_, '_> {
    type Error = !;
    const BLOB_LEN: usize = 0;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl ValidateBlob for Pile<'_, '_> {
    type Error = !;
    const BLOB_LEN: usize = 0;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<Z> Load<Z> for TryPile<'_, '_>
where Z: Borrow<Self>
{
    fn decode_blob<'a>(_: ValidBlob<'a, Self>, zone: &Z) -> Self {
        zone.borrow().clone()
    }
}

impl<Z> Load<Z> for Pile<'_, '_>
where Z: Borrow<Self>
{
    fn decode_blob<'a>(_: ValidBlob<'a, Self>, zone: &Z) -> Self {
        zone.borrow().clone()
    }
}


impl<'p,'v> Zone for Pile<'p, 'v> {
    type Ptr = Offset<'p, 'v>;
}

impl<'p, 'v> TryGet<Offset<'p, 'v>> for TryPile<'p, 'v> {
    type Error = ErrorKind;

    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a Offset<'p,'v>, metadata: T::Metadata) -> Result<Ref<'a, T>, Self::Error>
        where T: ?Sized + Load<Self>,
    {
        let blob_len = T::try_blob_len(metadata).map_err(|err| ErrorKind::Layout(err.into()))?;

        todo!()
    }

    unsafe fn try_take_unchecked<'a, T>(&self, ptr: Offset<'p,'v>, metadata: T::Metadata) -> Result<T::Owned, Self::Error>
        where T: ?Sized + Load<Self>
    {
        todo!()
    }
}


/*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TryPileMut<'pile, 'version>(TryPile<'pile, 'version>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileMut<'pile, 'version>(Pile<'pile, 'version>);

impl<'p> TryPile<'p, 'static> {
    pub fn empty() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        TryPile {
            marker: PhantomData,
            mapping: &EMPTY_SLICE,
        }
    }
}

impl<'p> Pile<'p, 'static> {
    pub fn empty() -> Self {
        Self(TryPile::empty())
    }
}


impl<'p> Default for TryPileMut<'p, 'static> {
    fn default() -> Self {
        Self(TryPile::empty())
    }
}

impl<'p> Default for PileMut<'p, 'static> {
    fn default() -> Self {
        Self(Pile::empty())
    }
}

impl<'p,'v> Zone for PileMut<'p, 'v> {
    type Ptr = OffsetMut<'p, 'v>;

    unsafe fn clone_ptr_unchecked<T: Clone>(ptr: &Self::Ptr) -> Self::Ptr {
        todo!()
    }

    fn alloc<T: ?Sized + Pointee>(_value: impl Take<T>) -> Bag<T, Self> {
        todo!()
    }
}

impl<'p, 'v> Get for PileMut<'p, 'v> {
    unsafe fn get_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a Self::Ptr, metadata: T::Metadata) -> Ref<'a, T>
        where T: Load<Self>,
    {
        match ptr.kind() {
            Kind::Ptr(ptr) => Ref::Ref(&*T::make_fat_ptr(ptr.cast().as_ptr(), metadata)),
            Kind::Offset(offset) => {
                todo!()
            },
        }
    }

    unsafe fn take_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: Self::Ptr, metadata: T::Metadata) -> T::Owned
        where T: Load<Self>
    {
        todo!()
    }
}

impl<'p, 'v> GetMut for PileMut<'p, 'v> {
    unsafe fn get_mut_unchecked<'a, T: ?Sized + Pointee>(&self, ptr: &'a mut Self::Ptr, metadata: T::Metadata) -> &'a mut T
        where T: Load<Self>,
    {
        match ptr.kind() {
            Kind::Ptr(ptr) => &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata),
            Kind::Offset(offset) => {
                todo!()
            },
        }
    }
}

/*
use std::mem;

use owned::Take;

use crate::pointee::Pointee;
use crate::ptr::*;
use crate::load::Load;
use crate::save::*;


pub mod offsetmut;
use self::offsetmut::OffsetMut;




impl<'p,'v> Get<Offset<'p, 'v>> for Pile<'p, 'v> {
    unsafe fn get_unchecked<'a, T: ?Sized + Load>(&self, ptr: &'a Offset<'p,'v>, metadata: T::Metadata) -> &'a T {
        todo!()
    }

    unsafe fn take_unchecked<T: Load>(&self, ptr: Offset<'p, 'v>, metadata: T::Metadata) -> T {
        todo!()
    }
}

impl<'p,'v> Get<OffsetMut<'p, 'v>> for Pile<'p, 'v> {
    unsafe fn get_unchecked<'a, T: ?Sized + Load>(&self, ptr: &'a OffsetMut<'p,'v>, metadata: T::Metadata) -> &'a T {
        todo!()
    }

    unsafe fn take_unchecked<T: Load>(&self, ptr: OffsetMut<'p, 'v>, metadata: T::Metadata) -> T {
        todo!()
    }
}



impl<'p> Default for Pile<'p, 'static> {
    fn default() -> Self {
        Self(TryPile::default())
    }
}

#[derive(Debug)]
pub struct ShallowDumper<'p, 'v> {
    pile: Pile<'p, 'v>,
    buf: Vec<u8>,
}

impl<'p, 'v> ShallowDumper<'p, 'v> {
    pub fn new(pile: Pile<'p, 'v>) -> Self {
        Self {
            pile,
            buf: vec![],
        }
    }

    pub fn save_tip<'a, T>(mut self, tip: &'a T) -> (Vec<u8>, Offset<'p, 'static>)
        where T: Save<'a, OffsetMut<'p, 'v>, Offset<'p, 'static>>
    {
        let mut state = tip.init_save_state();
        self = tip.save_poll(&mut state, self).into_ok();

        let (this, offset) = self.save(tip, &state).into_ok();
        (this.buf, offset)
    }
}

impl<'p, 'v1, 'v2> SavePtr<OffsetMut<'p, 'v1>, Offset<'p, 'v2>> for ShallowDumper<'p, 'v1> {
    type Error = !;

    fn save<'a, T: ?Sized>(mut self, value: &'a T, state: &T::State) -> Result<(Self, Offset<'p, 'v2>), Self::Error>
        where T: Save<'a, OffsetMut<'p, 'v1>, Offset<'p, 'v2>>
    {
        let offset = Offset::new(self.buf.len()).expect("overflow");
        let buf = mem::replace(&mut self.buf, vec![]);
        self.buf = value.save_blob(state, buf).into_ok();

        Ok((self, offset))
    }

    unsafe fn try_save_ptr<'a, T: ?Sized + Pointee>(&mut self, ptr: &'a OffsetMut<'p, 'v1>, metadata: T::Metadata)
        -> Result<Offset<'p, 'v2>, &'a T>
    {
        match ptr.try_get_dirty_unchecked::<T>(metadata) {
            Ok(value) => Err(value),
            Err(persist_ptr) => Ok(persist_ptr.cast()),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    use crate::heap::Heap;

    #[test]
    fn test() {
        let pile = Pile::default();

        let bag = pile.alloc([42u8, 43u8, 44u8]);
        let bag = Heap::alloc(bag);
        let bag = Heap::alloc(bag);
        let bag = Heap::alloc(bag);

        let dumper = ShallowDumper::new(pile);

        let (v, ptr) = dbg!(dumper.save_tip(&bag));
    }
}

/*
use std::any::type_name;
use std::cmp;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ops;
use std::ptr::{self, NonNull};
use std::slice;

use std::alloc::Layout;

use owned::{Take, IntoOwned};

use crate::coerce::{Coerce, TryCoerce};
use crate::pointee::Pointee;
use crate::zone::{*, refs::*};
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::load::*;
use crate::marshal::save::*;
use crate::marshal::blob::*;
use crate::marshal::*;


pub mod offsetmut;
use self::offsetmut::OffsetMut;

pub mod error;
use self::error::*;



impl<'p, 'v> From<TryPile<'p, 'v>> for Pile<'p,'v> {
    #[inline(always)]
    fn from(trypile: TryPile<'p,'v>) -> Self {
        Self(trypile)
    }
}

impl<'p, 'v> From<Pile<'p, 'v>> for TryPile<'p,'v> {
    #[inline(always)]
    fn from(pile: Pile<'p,'v>) -> Self {
        pile.0
    }
}


impl<'p,'v> ops::Deref for Pile<'p,'v> {
    type Target = TryPile<'p,'v>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


pub trait PileZone<'p, 'v>
: Zone<Error = Error<'p,'v>,
       PersistPtr = Offset<'static, 'static>>
{
    fn get_try_pile(&self) -> TryPile<'p, 'v>;

    fn mapping(&self) -> &'p dyn Mapping;

    fn slice(&self) -> &'p &'p [u8] {
        unsafe {
            &*(self.mapping() as *const dyn Mapping as *const &'p [u8])
        }
    }
}

pub trait PileZoneMut<'p, 'v> : PileZone<'p, 'v> + Zone<Ptr = OffsetMut<'p,'v>>
{}

impl<'p, 'v> PileZone<'p, 'v> for TryPile<'p, 'v> {
    #[inline(always)]
    fn get_try_pile(&self) -> TryPile<'p, 'v> {
        *self
    }

    #[inline(always)]
    fn mapping(&self) -> &'p dyn Mapping {
        self.mapping
    }
}

impl<'p, 'v> PileZone<'p, 'v> for TryPileMut<'p, 'v> {
    #[inline(always)]
    fn get_try_pile(&self) -> TryPile<'p, 'v> {
        self.0
    }

    #[inline(always)]
    fn mapping(&self) -> &'p dyn Mapping {
        self.0.mapping
    }
}

impl<'p, 'v> PileZoneMut<'p, 'v> for TryPileMut<'p, 'v> {}

impl<'p, 'v1, 'v2, T> AsRef<FatPtr<T, TryPile<'p, 'v2>>> for FatPtr<T, TryPile<'p, 'v1>>
where T: ?Sized + Pointee,
{
    #[inline(always)]
    fn as_ref(&self) -> &FatPtr<T, TryPile<'p, 'v2>> {
        unsafe { mem::transmute(self) }
    }
}

impl TryPile<'_, '_> {
    /// Creates a new `TryPile` from a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// # use leint::Le;
    /// TryPile::new([0x12, 0x34, 0x56, 0x78], |pile| {
    ///     let tip = pile.try_get_tip::<Le<u32>>().unwrap();
    ///     assert_eq!(**tip, 0x78563412);
    /// })
    /// ```
    #[inline]
    pub fn new<R>(slice: impl AsRef<[u8]>, f: impl FnOnce(TryPile) -> R) -> R {
        let slice = slice.as_ref();
        Unique::new(&slice, |slice| {
            f(TryPile::from(slice))
        })
    }
}

impl TryPile<'_, 'static> {
    /// Creates an empty `TryPile`.
    ///
    /// Note how the `'version` parameter is `'static': the earliest possible version of a pile is
    /// to have nothing in it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// let empty = TryPile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert!(empty.try_get_tip::<u8>().is_err());
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.try_get_tip::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        TryPile {
            marker: PhantomData,
            mapping: &EMPTY_SLICE,
        }
    }
}

impl<'p,'v> TryPile<'p, 'v> {
    pub fn new_valid_ptr<T: ?Sized + Pointee>(offset: usize, metadata: T::Metadata) -> ValidPtr<T, Self> {
        let raw = Offset::new(offset).unwrap();

        // Safe because our pointers have no special meaning.
        unsafe { ValidPtr::new_unchecked(FatPtr { raw, metadata }) }
    }

    /// Tries to get the tip of a `TryPile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// # use leint::Le;
    /// TryPile::new(&[42], |pile| {
    ///     let tip = pile.try_get_tip::<u8>().unwrap();
    ///     assert_eq!(**tip, 42u8);
    ///
    ///     // Fails, because the slice is too small to be a valid Le<u32>
    ///     assert!(pile.try_get_tip::<Le<u32>>().is_err());
    ///
    ///     // Fails, because the slice isn't a valid bool
    ///     assert!(pile.try_get_tip::<bool>().is_err());
    /// })
    /// ```
    pub fn try_get_tip<T: Decode<Self>>(&self) -> Result<Ref<'p, T, Self>, Error<'p,'v>> {
        // By using saturating_sub we don't have to handle the too-large case ourselves.
        let offset = self.slice().len().saturating_sub(mem::size_of::<T>());

        let ptr = FatPtr::<T,_> {
            raw: Offset::new(offset.into()).unwrap(),
            metadata: ()
        };
        let r = try_get_impl(self, &ptr)?;
        Ok(Ref {
            this: unsafe { T::assume_valid_ref(r) },
            zone: *self,
        })
    }
}

fn get_blob_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &FatPtr<T, Z::Persist>,
) -> Result<Blob<'a, T::Persist>, Error<'p,'v>>
where T: ?Sized + PersistPointee,
      Z: PileZone<'p,'v>,
{
    let layout = T::try_layout(ptr.metadata)
                   .map_err(|e| Error::new(zone, ptr, ErrorKind::Metadata(e.into())))?;

    // It's impossible for this to overflow as the maximum offset is just a quarter of
    // usize::MAX
    let start = ptr.raw.get();
    let end = start + layout.size();
    match zone.slice().get(start .. end) {
        None => Err(Error::new(zone, ptr, ErrorKind::Offset)),
        Some(slice) => {
            let ptr = T::Persist::make_fat_ptr(slice.as_ptr() as *const (), ptr.metadata);

            unsafe {
                assert_eq!(layout.align(), 1,
                           "PersistPointee can't be implemented for aligned type {}", type_name::<T>());
                Ok(Blob::from_ptr(ptr))
            }
        },
    }
}

fn try_get_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &FatPtr<T, Z::Persist>,
)
-> Result<&'a T::Persist, Error<'p, 'v>>
where T: ?Sized + PersistPointee,
      Z: PileZone<'p,'v>,
{
    let blob = get_blob_impl(zone, ptr)?;

    let cursor = blob.into_cursor_ignore_padding();
    match T::Persist::validate(cursor) {
        Ok(valid_blob) => Ok(valid_blob.to_ref()),
        Err(BlobError::Error(err)) => Err(Error::new(zone, ptr, ErrorKind::Value(err.into()))),
        Err(BlobError::Padding(never)) => match never {},
    }
}

impl Pile<'_, '_> {
    /// Creates a new `Pile` from a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::Pile;
    /// Pile::new([1,2,3,4], |pile| {
    /// })
    /// ```
    #[inline]
    pub fn new<R>(slice: impl AsRef<[u8]>, f: impl FnOnce(Pile) -> R) -> R {
        TryPile::new(slice, |try_pile| {
            f(Pile::from(try_pile))
        })
    }
}

impl Pile<'_, 'static> {
    /// Creates an empty `Pile`.
    ///
    /// Note how the `'version` parameter is `'static': the earliest possible version of a pile is
    /// to have nothing in it.
    ///
    /// # Examples
    ///
    /// ```todo
    /// # use hoard::pile::{Pile, TipError};
    /// let empty = Pile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert_eq!(empty.fully_validate_tip::<u8>().unwrap_err(),
    ///            TipError::Undersized);
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.fully_validate_tip::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        TryPile::empty().into()
    }
}

/// Validates piles fully.
#[derive(Debug)]
pub struct FullValidator<'p,'v, Z> {
    marker: PhantomData<TryPile<'p,'v>>,
    pile: Z,
}

impl<'p, 'v, Z> PtrValidator<Z> for FullValidator<'p, 'v, Z>
where Z: PileZone<'p, 'v>
{
    type Error = Error<'p,'v>;

    fn validate_ptr<'a, T: ?Sized>(
        &self,
        ptr: &'a Z::PersistPtr,
        metadata: T::Metadata,
    ) -> Result<Option<&'a T::Persist>, Self::Error>
        where T: ValidatePointeeChildren<'a, Z>
    {
        //let blob = get_blob_impl(&self.pile, ptr)?;

        /*
        match T::validate_blob(blob.into_validator()) {
            Ok(valid_blob) => Ok(Some(valid_blob.to_ref())),
            Err(e) => Err(PtrValidatorError::with_error(ptr, e)),
        }
        */ todo!()
    }
}

impl<'p,'v> Zone for TryPile<'p,'v> {
    type Ptr = Offset<'p,'v>;
    type Persist = TryPile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = Error<'p,'v>;

    #[inline(always)]
    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_ptr_unchecked<T>(ptr: &Offset<'p, 'v>) -> Offset<'p, 'v> {
        *ptr
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        Err(FatPtr {
            raw: ptr.raw.cast(),
            metadata: ptr.metadata,
        })
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let fat = owned.into_inner().into_inner();
        f(Err(FatPtr {
            raw: fat.raw.cast(),
            metadata: fat.metadata,
        }))
    }
}

impl<'p, 'v> TryGet for TryPile<'p, 'v> {
    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        /*
        let ptr: FatPtr<T, Self> = **ptr;
        let ptr: FatPtr<T, TryPile<'static, 'static>> = ptr.coerce();
        let r_persist = try_get_impl(self, &ptr)?;
        Ok(Ref {
            this: unsafe { T::assume_valid_ref(r_persist) },
            zone: *self,
        })
        */ todo!()
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        /*
        let ptr: FatPtr<T, Self> = **ptr;
        let ptr: FatPtr<T, TryPile<'static, 'static>> = ptr.coerce();
        let r_persist = try_get_impl(self, &ptr)?;

        Ok(Own {
            this: unsafe { T::assume_valid(r_persist) },
            zone: *self,
        })
        */ todo!()
    }
}

impl<'p,'v> Zone for Pile<'p,'v> {
    type Ptr = Offset<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = !;

    #[inline(always)]
    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_ptr_unchecked<T>(ptr: &Offset<'p, 'v>) -> Offset<'p, 'v> {
        *ptr
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        Err(FatPtr {
            raw: ptr.raw.cast(),
            metadata: ptr.metadata,
        })
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let fat = owned.into_inner().into_inner();
        f(Err(FatPtr {
            raw: fat.raw.cast(),
            metadata: fat.metadata,
        }))
    }
}

impl<'p, 'v> TryGet for Pile<'p, 'v> {
    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        todo!()
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: Offset<'p, 'v>, metadata: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        todo!()
    }
}

pub fn test_trypile<'a,'p,'v>(pile: &TryPile<'p,'v>,
    ptr1: &[ValidPtr<[bool;2], TryPile<'p, 'v>>; 100],
    ptr2: &[ValidPtr<u8, TryPile<'p, 'v>>; 100],
) -> Result<usize, Error<'p,'v>>
{
    let mut sum = 0;

    for (ptr1, ptr2) in ptr1.iter().zip(ptr2.iter()) {
        let [a,b] = **pile.try_get(ptr1)?;
        if a != b {
            let n = pile.try_get(ptr2)?;
            sum += **n as usize;
        }
    }
    Ok(sum)
}

impl<'p> Default for TryPileMut<'p, 'static> {
    fn default() -> Self {
        TryPileMut(TryPile::empty())
    }
}

impl<'p> Default for PileMut<'p, 'static> {
    fn default() -> Self {
        PileMut(TryPileMut::default())
    }
}

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl<'p,'v> Zone for TryPileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;
    type Persist = TryPile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = Error<'p,'v>;

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }

    #[inline(always)]
    fn duplicate(&self) -> Self {
        Self(self.0)
    }

    unsafe fn clone_ptr_unchecked<T>(ptr: &Self::Ptr) -> Self::Ptr {
        todo!()
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        match ptr.raw.kind() {
            offsetmut::Kind::Ptr(nonnull) => unsafe {
                Ok(&*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata))
            },
            offsetmut::Kind::Offset(raw) => {
                let raw = raw.cast();
                Err(FatPtr { raw, metadata: ptr.metadata })
            },
        }
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let metadata = owned.metadata;
        OffsetMut::try_take_dirty_unsized(owned, |r|
            f(match r {
                Ok(t_ref) => Ok(t_ref),
                Err(offset) => Err(FatPtr { raw: offset.cast(), metadata }),
            })
        )
    }
}

impl<'p,'v> Alloc for TryPileMut<'p,'v> {
    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }
}

impl<'p, 'v> TryGet for TryPileMut<'p, 'v> {
    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        /*
        let fatptr: FatPtr<T, Self> = **ptr;

        match TryCoerce::<FatPtr<T, TryPile>>::try_coerce(**ptr) {
            Ok(ptr) => {
                let r_persist = try_get_impl(self, &ptr)?;
                Ok(Ref {
                    this: unsafe { T::assume_valid_ref(r_persist) },
                    zone: *self,
                })
            },
            Err(err) => unsafe {
                let r: &T = &*T::make_fat_ptr(err.ptr.cast().as_ptr(), ptr.metadata);
                Ok(Ref {
                    this: r,
                    zone: self.duplicate()
                })
            },
        }
        */ todo!()
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        /*
        let metadata: T::Metadata = ptr.metadata;
        OffsetMut::try_take_dirty_unsized(ptr, |result| {
            match result {
                Ok(dirty) => {
                    Ok(Own {
                        this: unsafe { T::into_owned_unchecked(dirty) },
                        zone: *self,
                    })
                },
                Err(offset) => {
                    let ptr = FatPtr::<T, TryPile> {
                        raw: offset.coerce(),
                        metadata,
                    };
                    let r_persist = try_get_impl(self, &ptr)?;
                    Ok(Own {
                        this: unsafe { T::assume_valid(r_persist) },
                        zone: *self,
                    })
                },
            }
        })
        */ todo!()
    }
}

fn try_get_mut_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &'a mut ValidPtr<T,Z>,
)
-> Result<RefMut<'a, T, Z>, Error<'p, 'v>>
where T: ?Sized + PersistPointee,
      Z: PileZoneMut<'p,'v> + Alloc,
{
    match TryCoerce::<FatPtr<T, Z::Persist>>::try_coerce(**ptr) {
        Err(err) => {
            // Pointer is dirty, so we can just dereference it.
            let r: &mut T = unsafe { &mut *T::make_fat_ptr_mut(err.ptr.cast().as_ptr(), ptr.metadata) };
            Ok(RefMut {
                this: r,
                zone: zone.duplicate()
            })
        },
        Ok(persist_ptr) => {
            let r = try_get_impl(zone, &persist_ptr)?;

            // FIXME: double conversion, eg [u8] -> Vec[u8] -> [u8]
            //
            // Maybe assume_valid_alloc()?
            let owned = unsafe { T::assume_valid(r) };

            let new_ptr: FatPtr<T, Z> = zone.alloc(owned).into_inner().into_inner();

            // SAFETY: ValidPtr::raw_mut() requires us to maintain the validity of the pointer.
            // new_ptr is freshly allocated, so it should be valid as long as the metadata is
            // unchanged.
            assert_eq!(ptr.metadata, new_ptr.metadata);
            let old_raw = unsafe { mem::replace(ptr.raw_mut(), new_ptr.raw) };

            // Make sure we're not leaking memory.
            assert!(old_raw.get_offset().is_some());

            if let Some(nonnull) = ptr.raw.get_ptr() {
                Ok(RefMut {
                    // SAFETY: We do in fact have mutable access here.
                    this: unsafe { &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata) },
                    zone: zone.duplicate(),
                })
            } else {
                unreachable!("alloc should have returned a newly allocated ptr")
            }
        },
    }
}

impl<'p, 'v> TryGetMut for TryPileMut<'p, 'v> {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut OffsetMut<'p,'v>, metadata: T::Metadata)
        -> Result<RefMut<'a, T, Self>, Self::Error>
    {
        // try_get_mut_impl(self, ptr)
        todo!()
    }
}

impl<'p,'v> Zone for PileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = !;

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }

    #[inline(always)]
    fn duplicate(&self) -> Self {
        Self(self.0)
    }

    unsafe fn clone_ptr_unchecked<T>(ptr: &Self::Ptr) -> Self::Ptr {
        todo!()
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        match ptr.raw.kind() {
            offsetmut::Kind::Ptr(nonnull) => unsafe {
                Ok(&*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata))
            },
            offsetmut::Kind::Offset(raw) => {
                let raw = raw.cast();
                Err(FatPtr { raw, metadata: ptr.metadata })
            },
        }
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let metadata = owned.metadata;
        OffsetMut::try_take_dirty_unsized(owned, |r|
            f(match r {
                Ok(t_ref) => Ok(t_ref),
                Err(offset) => Err(FatPtr { raw: offset.cast(), metadata }),
            })
        )
    }
}

impl<'p,'v> Alloc for PileMut<'p,'v> {
    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }
}

impl<'p, 'v> TryGet for PileMut<'p, 'v> {
    unsafe fn try_get_unchecked<'a, T>(&self, ptr: &'a OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        match ptr.kind() {
            offsetmut::Kind::Offset(offset) => {
                todo!()
            },
            offsetmut::Kind::Ptr(nonnull) => {
                let r: &T = &*T::make_fat_ptr(nonnull.cast().as_ptr(), metadata);
                Ok(Ref {
                    this: r,
                    zone: self.duplicate()
                })
            }
        }
    }

    unsafe fn try_take_unchecked<T: ?Sized + Load<Self>>(&self, ptr: OffsetMut<'p, 'v>, metadata: T::Metadata)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        todo!()
    }
}

impl<'p, 'v> TryGetMut for PileMut<'p, 'v> {
    unsafe fn try_get_mut_unchecked<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut OffsetMut<'p,'v>, metadata: T::Metadata)
        -> Result<RefMut<'a, T, Self>, Self::Error>
    {
        todo!()
    }
}

#[derive(Debug)]
pub struct VecDumper<'a, 'p, 'v, Z> {
    marker: PhantomData<TryPile<'p, 'v>>,
    pile: Z,
    buf: &'a mut Vec<u8>,
}

impl<'a,'p,'v,Z> VecDumper<'a, 'p, 'v, Z> {
    pub fn new(pile: Z, buf: &'a mut Vec<u8>) -> Self {
        Self {
            marker: PhantomData,
            pile, buf,
        }
    }
}

impl<'a,'p,'v, Z> Dumper<Z> for VecDumper<'a,'p,'v, Z>
where Z: PileZone<'p, 'v>
{
    type Error = !;
    type BlobPtr = Offset<'static, 'static>;

    type WriteBlob = io::Cursor<&'a mut [MaybeUninit<u8>]>;
    type WriteBlobOk = &'a mut [u8];
    type WriteBlobError = !;

    fn try_save_ptr<'ptr, T: ?Sized + Pointee>(
        &self,
        ptr: &'ptr ValidPtr<T, Z>
    ) -> Result<Offset<'static, 'static>, &'ptr T>
    {
        match Z::try_get_dirty(ptr) {
            Ok(r) => Err(r),
            Err(ptr) => Ok(ptr.raw),
        }
    }

    fn save_blob(
        self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, Offset<'static, 'static>), !>
    {
        let offset = self.pile.slice().len() + self.buf.len();

        self.buf.reserve(size);

        let dst = unsafe { slice::from_raw_parts_mut(
            self.buf.as_mut_ptr().add(self.buf.len()) as *mut MaybeUninit<u8>,
            size
        )};
        f(io::Cursor::new(dst)).unwrap();

        unsafe { self.buf.set_len(self.buf.len() + size); }

        Ok((self, Offset::new(offset).unwrap()))
    }

    #[inline(always)]
    fn blob_ptr_to_zone_ptr(ptr: Self::BlobPtr) -> Z::PersistPtr {
        ptr
    }
}

impl<'p, 'v> TryPileMut<'p,'v> {
    pub fn encode_dirty<'a, T>(&self, value: &'a T) -> Vec<u8>
        where T: Encode<'a, Self>
    {
        let mut dst = vec![];

        let dumper = VecDumper::<Self>::new(*self, &mut dst);

        let mut state = value.make_encode_state();
        let dumper = value.encode_poll(&mut state, dumper).unwrap();

        let (_dumper, _offset) = dumper.encode_value(value, &state).unwrap();
        dst
    }
}

impl<'p,'v> SavePtr<Self> for TryPileMut<'p, 'v> {
    fn try_save_ptr<'a, T: ?Sized + Pointee, D>(ptr: &'a ValidPtr<T, Self>, dumper: &D)
        -> Result<Offset<'static, 'static>, &'a T>
        where D: Dumper<Self>
    {
        match Self::try_get_dirty(ptr) {
            Ok(r) => Err(r),
            Err(FatPtr { raw, metadata: _ }) => Ok(raw),
        }
    }
}
*/
*/

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    pub fn trypile_alloc() {
        let pile = TryPileMut::default();
        //let x = pile.alloc(42u8);

        /*
        assert_eq!(pile.encode_dirty(&x),
                   &[42,
                      1, 0, 0, 0, 0, 0, 0, 0]);

        let x = [pile.alloc(1u8), pile.alloc(2u8), pile.alloc(3u8)];
        assert_eq!(pile.encode_dirty(&x),
                   &[1, 2, 3,
                     1, 0, 0, 0, 0, 0, 0, 0,
                     3, 0, 0, 0, 0, 0, 0, 0,
                     5, 0, 0, 0, 0, 0, 0, 0,
                    ]);

        let x = [[pile.alloc(1u8), pile.alloc(2u8), pile.alloc(3u8)],
                 [pile.alloc(4u8), pile.alloc(5u8), pile.alloc(6u8)]];
        assert_eq!(pile.encode_dirty(&x),
                   &[ 1, 2, 3, 4, 5, 6,
                      1, 0, 0, 0, 0, 0, 0, 0,
                      3, 0, 0, 0, 0, 0, 0, 0,
                      5, 0, 0, 0, 0, 0, 0, 0,
                      7, 0, 0, 0, 0, 0, 0, 0,
                      9, 0, 0, 0, 0, 0, 0, 0,
                     11, 0, 0, 0, 0, 0, 0, 0,
                    ][..]);

        let x = pile.alloc(x);
        assert_eq!(pile.encode_dirty(&x),
                   &[ 1, 2, 3, 4, 5, 6,
                      1, 0, 0, 0, 0, 0, 0, 0,
                      3, 0, 0, 0, 0, 0, 0, 0,
                      5, 0, 0, 0, 0, 0, 0, 0,
                      7, 0, 0, 0, 0, 0, 0, 0,
                      9, 0, 0, 0, 0, 0, 0, 0,
                     11, 0, 0, 0, 0, 0, 0, 0,
                     13, 0, 0, 0, 0, 0, 0, 0,
                    ][..]);

        let x = pile.alloc(x);
        assert_eq!(pile.encode_dirty(&x),
                   &[ 1, 2, 3, 4, 5, 6,
                      1, 0, 0, 0, 0, 0, 0, 0,
                      3, 0, 0, 0, 0, 0, 0, 0,
                      5, 0, 0, 0, 0, 0, 0, 0,
                      7, 0, 0, 0, 0, 0, 0, 0,
                      9, 0, 0, 0, 0, 0, 0, 0,
                     11, 0, 0, 0, 0, 0, 0, 0,
                     13, 0, 0, 0, 0, 0, 0, 0,
                    109, 0, 0, 0, 0, 0, 0, 0,
                    ][..]);
        */
    }
}
*/
*/
*/
