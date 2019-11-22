use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops;
use core::ptr::NonNull;
use core::pin::Pin;

use super::*;

use crate::marshal::{*, blob::*};

use crate::coerce::TryCastRef;

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
        let blob = Offset::get_blob_from_pile(ptr, self)
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
}

impl<'s,'m> LoadPtr<Self> for PileMut<'s,'m> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'s,'m>>)
        -> FullyValidBlob<'a, T, Self>
    where T: ?Sized + Load<Self>
    {
        OffsetMut::load_valid_blob_from_pile(ptr, self)
            .expect("ValidPtr to be valid")
    }
}

/*

impl<'s,'p> ValidatePtr<OffsetMut<'s,'p>> for PileMut<'s,'p> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, OffsetMut<'s,'p>>)
        -> Result<Option<BlobValidator<'a, T, OffsetMut<'s,'p>>>, Self::Error>
    where T: ?Sized + Load<OffsetMut<'s,'p>>
    {
        match ptr.raw.kind() {
            Kind::Ptr(_) => Ok(None),
            Kind::Offset(offset) => {
                let size = T::dyn_blob_layout(ptr.metadata).size();
                let blob = self.get_blob(&offset, size)
                               .ok_or_else(||
                                    ValidatePtrError::Offset {
                                        offset: offset.to_static(),
                                        size,
                                    }
                               )?;
                let blob = Blob::new(blob, ptr.metadata).unwrap();

                T::validate_blob(blob).map(Some)
                    .map_err(|e| ValidatePtrError::Value(Box::new(e)))
            }
        }
    }
}


impl<'s,'p> LoadPtr<OffsetMut<'s,'p>> for PileMut<'s,'p> {
    fn load_blob<'a, T: ?Sized + Load<OffsetMut<'s,'p>>>(&self, ptr: &'a ValidPtr<T, OffsetMut<'s,'p>>)
        -> FullyValidBlob<'a, T, OffsetMut<'s,'p>>
    {
        match ptr.raw.kind() {
            Kind::Ptr(_) => panic!(),
            Kind::Offset(offset) => {
                let blob = self.get_blob(&offset, T::dyn_blob_layout(ptr.metadata).size())
                               .expect("invalid ValidPtr");
                let blob = Blob::new(blob, ptr.metadata).unwrap();

                unsafe { blob.assume_fully_valid() }
            },
        }
    }
}
*/

#[derive(Debug)]
pub struct PileMut<'s, 'm>(Pile<'s, 'm>);

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

impl<'s,'m> BlobZone for Pile<'s,'m> {
    type BlobPtr = Offset<'s,'m>;
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

impl<'s,'p> BlobZone for PileMut<'s,'p> {
    type BlobPtr = Offset<'s,'p>;
}

impl<'s,'p> Alloc for PileMut<'s,'p> {
    type Zone = Self;
    type Ptr = OffsetMut<'s,'p>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::new_unchecked(ValidPtr::<T,_>::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata }))
            //Own::new_unchecked(ValidPtr::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata }))
        })
    }

    fn zone(&self) -> Self::Zone {
        Self(self.0.clone())
    }
}

impl<'s,'m> Get for Pile<'s, 'm> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a Own<T, Self::Ptr>) -> Ref<'a, T>
        where Self: 'a
    {
        let blob = self.load_blob(ptr);
        T::load_blob(blob, self)
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
        let blob = self.load_blob(&ptr);
        T::decode_blob(blob, self)
    }
}

impl<'s,'m> Get for PileMut<'s, 'm> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, own: &'a Own<T, Self::Ptr>) -> Ref<'a, T>
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

    fn take<T: ?Sized + Load<Self>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
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
        let owned: Own<u8, OffsetMut<'static,'_>> = alloc.alloc(42u8);

        fn test_get<'a,'s,'m>(pile: &PileMut<'s,'m>, ptr: &'a Own<u8, OffsetMut<'static, 'm>>) -> Ref<'a,u8> {
            pile.get(ptr)
        }

        assert_eq!(*test_get(&alloc.zone(), &owned), 42);

        let snapshot = unsafe { Snapshot::new_unchecked(vec![]) };
        let pile = PileMut::from(Pile::new(&snapshot));

        assert_eq!(*test_get(&pile, &owned), 42);
    }
}
