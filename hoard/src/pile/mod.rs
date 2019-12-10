use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops;
use core::ptr::NonNull;
use core::pin::Pin;

use leint::Le;

use super::*;

use crate::marshal::{*, blob::*};

use crate::coerce::{TryCastRef, CastRef};

pub mod offset;
use self::offset::Kind;
pub use self::offset::{Offset, OffsetMut};

mod snapshot;
pub use self::snapshot::{Snapshot, Mapping};

#[derive(Debug, Clone, Copy)]
pub struct Pile<'s, 'm> {
    marker: PhantomData<fn(&'s ())>,
    snapshot: NonNull<Snapshot<'m>>,
}

impl<'s, 'm> Pile<'s, 'm> {
    pub fn new(snapshot: &'s Snapshot<'m>) -> Self {
        Self {
            marker: PhantomData,
            snapshot: NonNull::from(snapshot),
        }
    }

    pub fn get_blob<'p, T>(&self, ptr: &'p FatPtr<T, Offset<'s, 'm>>) -> Result<Blob<'p, T, Self>, offset::OffsetError>
        where T: ?Sized + Load<Self>
    {
        Offset::get_blob_from_pile(ptr, self)
    }
}

#[derive(Debug)]
pub enum ValidatePtrError {
    Offset(offset::OffsetError),
    Value(Box<dyn crate::marshal::Error>),
}

impl<'s,'m> ValidatePtr<Self> for Pile<'s,'m> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'s,'m>>)
        -> Result<Option<BlobValidator<'a, T, Self>>, Self::Error>
    where T: ?Sized + Load<Self>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'s,'m> ValidatePtr<Self> for PileMut<'s,'m> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'s,'m>>)
        -> Result<Option<BlobValidator<'a, T, Self>>, Self::Error>
    where T: ?Sized + Load<Self>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'s,'m> LoadPtr<Self> for Pile<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, Self>
    where T: ?Sized + Load<Self>
    {
        Offset::load_valid_blob_from_pile(ptr, self)
            .expect("ValidPtr to be valid")
    }

    fn blob_zone(&self) -> &Self {
        self
    }
}

impl<'s,'m> LoadPtr<Self> for PileMut<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, Self>
    where T: ?Sized + Load<Self>
    {
        OffsetMut::load_valid_blob_from_pile(ptr, self)
            .expect("ValidPtr to be valid")
    }

    fn blob_zone(&self) -> &Self {
        self
    }
}

#[derive(Debug)]
pub struct PileMut<'s, 'm>(Pile<'s, 'm>);

impl<'s, 'm> PileMut<'s, 'm> {
    pub fn get_blob<'p, T>(&self, ptr: &'p FatPtr<T, Offset<'s, 'm>>) -> Result<Blob<'p, T, Self>, offset::OffsetError>
        where T: ?Sized + Load<Self>
    {
        OffsetMut::get_blob_from_pile(ptr, self)
    }
}

impl<'s,'m> From<Pile<'s,'m>> for PileMut<'s,'m> {
    fn from(pile: Pile<'s,'m>) -> Self {
        Self(pile)
    }
}

impl<'s,'m> ops::Deref for PileMut<'s,'m> {
    type Target = Pile<'s,'m>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for PileMut<'static, '_> {
    fn default() -> Self {
        let pile = Pile::new(&snapshot::EMPTY_SNAPSHOT);
        Self(pile)
    }
}

impl<'s,'p> Zone for Pile<'s,'p> {
    type Ptr = Offset<'s,'p>;

    type Allocator = crate::never::NeverAllocator<Self>;

    fn allocator() -> Self::Allocator where Self: Default {
        unreachable!()
    }
}

impl<'s,'p> Zone for PileMut<'s,'p> {
    type Ptr = OffsetMut<'s,'p>;

    type Allocator = Self;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        Self::default()
    }
}

impl<'s,'p> Alloc for PileMut<'s,'p> {
    type Ptr = OffsetMut<'s,'p>;
    type Zone = Self;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, OffsetMut<'s,'p>> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata }))
        })
    }

    fn zone(&self) -> Self::Zone {
        Self(self.0.clone())
    }
}

impl<'s,'m> Get for Pile<'s, 'm> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self::Ptr>) -> Ref<'a, T>
        where Self: 'a
    {
        let blob = self.load_blob(ptr);
        T::load_blob(blob, self)
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self::Ptr>) -> T::Owned {
        let blob = self.load_blob(&ptr);
        T::decode_blob(blob, self)
    }
}

impl<'s,'m> Get for PileMut<'s, 'm> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, own: &'a ValidPtr<T, Self::Ptr>) -> Ref<'a, T>
        where Self: 'a
    {
        match own.raw.kind() {
            Kind::Offset(_) => {
                let ptr: &'a ValidPtr<T, Offset<'s,'m>> = own.try_cast_ref().unwrap();
                let blob = self.load_blob(ptr);
                T::load_blob(blob, self)
            },
            Kind::Ptr(ptr) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(ptr.cast().as_ptr(), own.metadata)
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self::Ptr>) -> T::Owned {
        let ptr = ptr.into_inner();
        if let Ok(valid_offset_ptr) = ptr.try_cast_ref() {
                let blob = self.load_blob(valid_offset_ptr);
                T::decode_blob(blob, self)
        } else {
            let FatPtr { raw, metadata } = ptr.into_inner();
            match unsafe { raw.try_take::<T>(metadata) } {
                Ok(owned) => owned,
                Err(_offset) => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Default)]
    struct DeepDump {
        dst: Vec<u8>,
    }

    impl<'s,'m> Dumper<Pile<'s,'m>> for DeepDump {
        type Pending = !;

        fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, Offset<'s,'m>>)
            -> Result<Offset<'s,'m>, &'p T>
        {
            Ok(ptr.raw)
        }

        fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s,'m>), !> {
            let offset = self.dst.len();
            self.dst.resize(offset + size, 0);

            f(&mut self.dst[offset..]);

            let offset = unsafe { Offset::new_unchecked(offset) };
            Ok((self, offset))
        }
    }

    #[derive(Debug, Default)]
    struct DeepDumpMut {
        dst: Vec<u8>,
    }

    impl<'s,'m> Dumper<PileMut<'s,'m>> for DeepDumpMut {
        type Pending = !;

        fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, OffsetMut<'s,'m>>)
            -> Result<Offset<'s,'m>, &'p T>
        {
            match ptr.raw.kind() {
                Kind::Offset(offset) => Ok(offset),
                Kind::Ptr(nonnull) => {
                    let r: &'p T = unsafe {
                        &*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata)
                    };
                    Err(r)
                },
            }
        }

        fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s,'m>), !> {
            let offset = self.dst.len();
            self.dst.resize(offset + size, 0);

            f(&mut self.dst[offset..]);

            let offset = unsafe { Offset::new_unchecked(offset) };
            Ok((self, offset))
        }
    }

    #[test]
    fn deepdump_pile() {
        let snapshot = unsafe { Snapshot::new_unchecked(vec![0x78,0x56,0x34,0x12]) };
        let pile = Pile::new(&snapshot);

        let offset = Offset::new(&snapshot, 0, 0).unwrap();
        let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
        let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

        let dumper = DeepDump::default();

        let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
        assert_eq!(dumper.dst, &[]);
    }

    #[test]
    fn deepdump_pilemut() {
        let snapshot = unsafe { Snapshot::new_unchecked(vec![0x78,0x56,0x34,0x12]) };
        let mut pile = PileMut::from(Pile::new(&snapshot));

        let offset = OffsetMut::from(Offset::new(&snapshot, 0, 0).unwrap());
        let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
        let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

        let dumper = DeepDumpMut::default();

        let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
        assert_eq!(dumper.dst, &[]);

        let owned_ptr = pile.alloc(0xabcd_u16);
        let r = dumper.try_save_ptr(&owned_ptr).unwrap_err();
        assert_eq!(r, &0xabcd);
        assert_eq!(dumper.dst, &[]);
    }

    /*
    #[test]
    fn pile_load() {
        let snapshot = unsafe { Snapshot::new_unchecked(vec![1,2,3,4]) };
        let pile = Pile::new(&snapshot);

        let offset = Offset::new(&snapshot, 0, 0).unwrap();
        let fatptr: FatPtr<(),_> = FatPtr { raw: offset, metadata: () };
        let validptr = unsafe { ValidPtr::new_unchecked(fatptr) };

        let blob = pile.load_blob(&validptr);
        assert_eq!(blob.len(), 0);

        let offset = Offset::new(&snapshot, 2, 2).unwrap();
        let fatptr: FatPtr<u16,_> = FatPtr { raw: offset, metadata: () };
        let validptr = unsafe { ValidPtr::new_unchecked(fatptr) };

        let blob = pile.load_blob(&validptr);
        assert_eq!(&blob[..], &[3,4]);
    }

    #[test]
    fn pile_load_owned_ptr() {
        let mapping = vec![42, 1,0,0,0,0,0,0,0];
        let snapshot = unsafe { Snapshot::new_unchecked(mapping) };
        let pile = Pile::new(&snapshot);

        let offset = Offset::new(&snapshot, 1, 8).unwrap();
        let fatptr: FatPtr<OwnedPtr<u8, Offset>, Offset> = FatPtr { raw: offset, metadata: () };
        let validptr = unsafe { ValidPtr::new_unchecked(fatptr) };

        let blob = pile.load_blob(&validptr);

        let loaded = <OwnedPtr<u8, Offset> as Decode<Pile<'_,'_>>>::load_blob(blob, &pile);
        assert_eq!(loaded.raw.get(), 0);

        assert_eq!(*pile.get(&loaded), 42);
    }

    #[test]
    fn pile_save_owned_ptr() {
        let mapping = vec![1,0,0,0,0,0,0,0];
        let snapshot = unsafe { Snapshot::new_unchecked(mapping) };
        let pile = Pile::new(&snapshot);

        let offset = Offset::new(&snapshot, 0, 8).unwrap();
        let fatptr: FatPtr<OwnedPtr<(), Offset>, Offset> = FatPtr { raw: offset, metadata: () };
        let validptr = unsafe { ValidPtr::new_unchecked(fatptr) };

        let owned: OwnedPtr<OwnedPtr<(), Offset>, Offset> = unsafe { OwnedPtr::new_unchecked(validptr) };

        let _state = <OwnedPtr<OwnedPtr<(), Offset>, Offset> as Encode<Pile>>::init_encode_state(&owned);
    }

    #[test]
    fn pilemut_get_owned() {
        let mut alloc = PileMut::allocator();
        let pile = alloc.zone();

        let owned = alloc.alloc(42u8);
        if let Ref::Borrowed(n) = pile.get(&owned) {
            assert_eq!(*n, 42);
        } else {
            panic!()
        }
        assert_eq!(pile.take(owned), 42);
    }

    #[test]
    fn pilemut_get_lifetimes() {
        let mut alloc = PileMut::allocator();
        let owned: OwnedPtr<u8, OffsetMut<'static,'_>> = alloc.alloc(42u8);

        fn test_get<'a,'s,'m>(pile: &PileMut<'s,'m>, ptr: &'a OwnedPtr<u8, OffsetMut<'static, 'm>>) -> Ref<'a,u8> {
            pile.get(ptr)
        }

        assert_eq!(*test_get(&alloc.zone(), &owned), 42);

        let snapshot = unsafe { Snapshot::new_unchecked(vec![]) };
        let pile = PileMut::from(Pile::new(&snapshot));

        assert_eq!(*test_get(&pile, &owned), 42);
    }
    */
}
