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

    pub fn get_blob<'p, T>(&self, ptr: &'p FatPtr<T, Offset<'s, 'm>>) -> Result<Blob<'p, T, Offset<'s,'m>>, offset::OffsetError>
        where T: ?Sized + Load<Offset<'s,'m>>
    {
        Offset::get_blob_from_pile(ptr, self)
    }
}

#[derive(Debug)]
pub enum ValidatePtrError {
    Offset(offset::OffsetError),
    Value(Box<dyn crate::marshal::Error>),
}

impl<'s,'m> ValidatePtr<Offset<'s,'m>> for Pile<'s,'m> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'s,'m>>)
        -> Result<Option<BlobValidator<'a, T, Offset<'s,'m>>>, Self::Error>
    where T: ?Sized + Load<Offset<'s,'m>>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'s,'m> ValidatePtr<OffsetMut<'s,'m>> for PileMut<'s,'m> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'s,'m>>)
        -> Result<Option<BlobValidator<'a, T, OffsetMut<'s,'m>>>, Self::Error>
    where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'s,'m> Loader<Offset<'s,'m>> for Pile<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, Offset<'s,'m>>
    where T: ?Sized + Load<Offset<'s,'m>>
    {
        Offset::load_valid_blob_from_pile(ptr, self)
            .expect("ValidPtr to be valid")
    }
}

impl<'s,'m> Loader<OffsetMut<'s,'m>> for PileMut<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, OffsetMut<'s,'m>>
    where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
        OffsetMut::load_valid_blob_from_pile(ptr, self)
            .expect("ValidPtr to be valid")
    }
}

#[derive(Debug)]
pub struct PileMut<'s, 'm>(Pile<'s, 'm>);

impl<'s, 'm> PileMut<'s, 'm> {
    pub fn get_blob<'p, T>(&self, ptr: &'p FatPtr<T, Offset<'s, 'm>>) -> Result<Blob<'p, T, OffsetMut<'s,'m>>, offset::OffsetError>
        where T: ?Sized + Load<OffsetMut<'s,'m>>
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

impl<'s,'p> Alloc for PileMut<'s,'p> {
    type Ptr = OffsetMut<'s,'p>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, OffsetMut<'s,'p>> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata }))
        })
    }

    fn zone(&self) -> PileMut<'s,'p> {
        Self(self.0.clone())
    }
}

impl<'s,'m> Get<Offset<'s, 'm>> for Pile<'s, 'm> {
    fn get<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>) -> Ref<'a, T>
        where T: ?Sized + Load<Offset<'s,'m>>
    {
        let blob = self.load_blob(ptr);
        T::load_blob(blob, self)
    }

    fn take<T>(&self, ptr: OwnedPtr<T, Offset<'s,'m>>) -> T::Owned
        where T: ?Sized + Load<Offset<'s,'m>>
    {
        let blob = self.load_blob(&ptr);
        T::decode_blob(blob, self)
    }
}

impl<'s,'m> Get<OffsetMut<'s,'m>> for PileMut<'s, 'm> {
    fn get<'a, T>(&self, own: &'a ValidPtr<T, OffsetMut<'s,'m>>) -> Ref<'a, T>
        where T: ?Sized + Load<OffsetMut<'s,'m>>
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

    fn take<T>(&self, ptr: OwnedPtr<T, OffsetMut<'s,'m>>) -> T::Owned
        where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
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

impl<'s,'m> GetMut<OffsetMut<'s,'m>> for PileMut<'s, 'm> {
    fn get_mut<'a, T>(&self, ptr: &'a mut ValidPtr<T, OffsetMut<'s,'m>>) -> &'a mut T
        where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
        let metadata = ptr.metadata;

        match ptr.raw.kind() {
            Kind::Offset(_) => {
                todo!()
            },
            Kind::Ptr(ptr) => {
                unsafe {
                    &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata)
                }
            },
        }
    }
}

pub fn save_to_vec<'s, 'm, T: ?Sized>(value: &T) -> Vec<u8>
where T: Save<OffsetMut<'s,'m>>
{
    let mut state = value.init_save_state();
    let (dumper, offset) = value.save_poll(&mut state, VecDumper::default()).unwrap();
    dumper.0
}

#[derive(Debug, Default)]
struct VecDumper(Vec<u8>);

impl<'s, 'm> Dumper<OffsetMut<'s,'m>> for VecDumper {
    type Pending = !;

    fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, OffsetMut<'s, 'm>>) -> Result<Offset<'s,'m>, &'p T> {
        match ptr.raw.kind() {
            Kind::Offset(offset) => Ok(offset),
            Kind::Ptr(nonnull) => {
                let r: &'p T = unsafe {
                    &*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata)
                };
                Err(r)
            }
        }
    }

    fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s,'m>), !> {
        let offset = self.0.len();

        self.0.resize(offset + size, 0);
        f(&mut self.0[offset ..]);

        let offset = unsafe { Offset::new_unchecked(offset) };
        Ok((self, offset))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Default)]
    struct DeepDump {
        dst: Vec<u8>,
    }

    impl<'s,'m> Dumper<Offset<'s,'m>> for DeepDump {
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

    impl<'s,'m> Dumper<OffsetMut<'s,'m>> for DeepDumpMut {
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

    #[test]
    fn pilemut_getmut() {
        let mut zone = PileMut::default();
        let mut owned = zone.alloc((1,2,3));

        let r = zone.get_mut(&mut owned);

        assert_eq!(*r, (1,2,3));
        r.0 = 10;
        assert_eq!(*r, (10,2,3));

        let r = zone.get_mut(&mut owned);
        assert_eq!(*r, (10,2,3));
    }
}
