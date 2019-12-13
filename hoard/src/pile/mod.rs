//! Append-only, copy-on-write, persistence via byte slices and offsets.
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

use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops;
use core::ptr::NonNull;
use core::pin::Pin;

use leint::Le;
use singlelife::Unique;

use super::*;

use crate::marshal::{*, blob::*};

use crate::coerce::{TryCastRef, CastRef};

pub mod offset;
use self::offset::Kind;
pub use self::offset::{Offset, OffsetMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pile<'s, 'p> {
    marker: PhantomData<
        for<'a> fn(&'a Offset<'s, 'p>) -> &'a [u8]
    >,
    slice: &'p &'p [u8],
}

impl<'s, 'p> Pile<'s, 'p> {
    pub fn new(slice: Unique<'p, &&[u8]>) -> Self
        where 's: 'p
    {
        Self {
            marker: PhantomData,
            slice: Unique::into_inner(slice),
        }
    }

    /*
    pub unsafe fn update<'s2>(&self, slice: &'s2 &[u8]) -> Pile<'s2, 'p>
        where 's: 's2
    {
        Pile::new_unchecked(slice)
    }
    */

    fn get_slice<'a>(&self, offset: &'a Offset<'s, 'p>, size: usize) -> Result<&'a [u8], offset::OffsetError> {
        let start = offset.get();

        // It's impossible for this to overflow as the maximum offset is just a quarter of
        // usize::MAX
        let end = start + size;
        self.slice.get(start .. end)
             .ok_or(offset::OffsetError { offset: start, size })
    }

    pub fn get_blob<'a, T>(&self, ptr: &'a FatPtr<T, Offset<'s, 'p>>) -> Result<Blob<'a, T, Offset<'s,'p>>, offset::OffsetError>
        where T: ?Sized + Load<Offset<'s, 'p>>
    {
        self.get_slice(&ptr.raw, T::dyn_blob_layout(ptr.metadata).size())
                  .map(|slice| Blob::new(slice, ptr.metadata).unwrap())
    }

    pub fn new_offset(&self, offset: usize) -> Option<Offset<'s, 'p>> {
        Offset::new(offset)
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
        self.get_blob(ptr)
            .map(|blob| unsafe { blob.assume_fully_valid() })
            .expect("ValidPtr to be valid")
    }
}

impl<'s,'m> Loader<OffsetMut<'s,'m>> for PileMut<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, OffsetMut<'s,'m>>
    where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
        self.get_blob(ptr)
            .map(|blob| unsafe { blob.assume_fully_valid() })
            .expect("ValidPtr to be valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileMut<'s, 'm>(Pile<'s, 'm>);

impl<'s, 'm> PileMut<'s, 'm> {
    pub fn get_blob<'p, T>(&self, ptr: &'p FatPtr<T, Offset<'s, 'm>>) -> Result<Blob<'p, T, OffsetMut<'s,'m>>, offset::OffsetError>
        where T: ?Sized + Load<OffsetMut<'s,'m>>
    {
        self.get_slice(&ptr.raw, T::dyn_blob_layout(ptr.metadata).size())
                  .map(|slice| Blob::new(slice, ptr.metadata).unwrap())
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
        static EMPTY_SLICE: &[u8] = &[];

        let slice = unsafe { Unique::new_unchecked(&EMPTY_SLICE) };
        let pile = Pile::new(slice);
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

        Ok((self, Offset::new(offset).unwrap()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use singlelife::unique;

/*
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

            let offset = Offset::new(offset).unwrap();
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

            let offset = Offset::new(offset).unwrap();
            Ok((self, offset))
        }
    }

    #[test]
    fn deepdump_pile() {
        let slice = &vec![0x78,0x56,0x34,0x12][..];
        unique!(|&slice| {
            let pile = Pile::new(slice);

            let offset = Offset::new(0).unwrap();
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            let dumper = DeepDump::default();

            let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
            assert_eq!(dumper.dst, &[]);
        })
    }

    #[test]
    fn deepdump_pilemut() {
        Unique::new(&[0x78,0x56,0x34,0x12][..], |pile| {
            let mut pile = PileMut::from(Pile::new(&pile));

            let offset = OffsetMut::from(Offset::new(0).unwrap());
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            let dumper = DeepDumpMut::default();

            let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
            assert_eq!(dumper.dst, &[]);

            let owned_ptr = pile.alloc(0xabcd_u16);
            let r = dumper.try_save_ptr(&owned_ptr).unwrap_err();
            assert_eq!(r, &0xabcd);
            assert_eq!(dumper.dst, &[]);
        })
    }
*/

    #[test]
    fn pilemut_get() {
        Unique::new(&&[0x78,0x56,0x34,0x12][..], |pile| {
            let mut static_pile = PileMut::default();
            let static_owned = static_pile.alloc(12u8);

            let pile = PileMut::from(Pile::new(pile));

            let offset = pile.new_offset(0).unwrap();
            let offset = OffsetMut::from(offset);
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            assert_eq!(*pile.get(&owned), 0x12345678);

            assert_eq!(*pile.get(&static_owned), 12);
            assert_eq!(*static_pile.get(&static_owned), 12);

            /*
            let buf = &[0x78,0x56,0x34,0x12,0x42][..];
            (||{
                let pile2 = unsafe { pile.update(&buf) };
                let pile2 = PileMut::from(pile2);

                assert_eq!(*pile2.get(&static_owned), 12);
                assert_eq!(*pile2.get(&owned), 0x12345678);

                let offset = pile2.new_offset(4).unwrap();
                let offset = OffsetMut::from(offset);
                let fatptr: FatPtr<u8,_> = FatPtr { raw: offset, metadata: () };
                let owned2 = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };
                assert_eq!(*pile2.get(&owned2), 0x42);

                // doesn't compile as pile is out of date
                //assert_eq!(*pile.get(&owned2), 0x42);
            })()
            */
        })
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

    #[test]
    fn offsetmut_clone() {
        let mut zone = PileMut::default();
        let v1 = zone.alloc((1,2,3));
        let v2 = v1.clone();
        assert_ne!(v1.raw, v2.raw);
    }
}
