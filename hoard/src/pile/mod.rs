use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops;

use super::*;

use crate::marshal::{*, blob::*};

mod offset;
use self::offset::Kind;
pub use self::offset::{Offset, OffsetMut};

#[derive(Debug, Clone, Copy)]
pub struct Pile<'s, 'p> {
    marker: PhantomData<fn(&'p [u8]) -> &'p [u8]>,
    slice: &'s &'s [u8],
}

impl<'s, 'p> Pile<'s, 'p> {
    pub unsafe fn new_unchecked(slice: &'s &[u8]) -> Self {
        Self { marker: PhantomData, slice }
    }

    pub fn get_blob(&self, offset: &Offset<'s, 'p>, len: usize) -> Option<&'s [u8]> {
        let start = usize::from(*offset);
        self.slice.get(start .. start + len)
    }

    pub fn root_offset<'a>(&'a self) -> Offset<'a, 'p> {
        Offset::new(0).unwrap()
    }
}


pub enum ValidatePtrError {
    Offset {
        offset: Offset<'static, 'static>,
        size: usize,
    },
    Value(Box<dyn crate::marshal::Error>),
}

impl<'s,'p> ValidatePtr<Offset<'s,'p>> for Pile<'s,'p> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'s,'p>>)
        -> Result<Option<BlobValidator<'a, T, Offset<'s,'p>>>, Self::Error>
    where T: ?Sized + Load<Offset<'s,'p>>
    {
        let size = T::dyn_blob_layout(ptr.metadata).size();
        let blob = self.get_blob(&ptr.raw, size)
                       .ok_or_else(||
                            ValidatePtrError::Offset {
                                offset: ptr.raw.to_static(),
                                size,
                            }
                       )?;
        let blob = Blob::new(blob, ptr.metadata).unwrap();

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

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

impl<'s,'p> LoadPtr<Offset<'s,'p>> for Pile<'s,'p> {
    fn load_blob<'a, T: ?Sized + Load<Offset<'s,'p>>>(&self, ptr: &'a ValidPtr<T, Offset<'s,'p>>)
        -> FullyValidBlob<'a, T, Offset<'s,'p>>
    {
        let blob = self.get_blob(&ptr.raw, T::dyn_blob_layout(ptr.metadata).size())
                       .expect("invalid ValidPtr");
        let blob = Blob::new(blob, ptr.metadata).unwrap();

        unsafe { blob.assume_fully_valid() }
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

#[derive(Debug)]
pub struct PileMut<'s, 'p>(Pile<'s, 'p>);

impl<'s,'p> From<Pile<'s,'p>> for PileMut<'s,'p> {
    fn from(pile: Pile<'s,'p>) -> Self {
        Self(pile)
    }
}

impl<'s,'p> ops::Deref for PileMut<'s,'p> {
    type Target = Pile<'s,'p>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for PileMut<'static, '_> {
    fn default() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        let pile = unsafe { Pile::new_unchecked(&EMPTY_SLICE) };
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
    type Zone = Self;
    type Ptr = OffsetMut<'s,'p>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        /*
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            Own::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata })
        })
        */
        todo!()
    }

    fn zone(&self) -> Self::Zone {
        Self(self.0.clone())
    }
}

impl Get for PileMut<'_, '_> {
    fn get<'a, T: ?Sized + Load<Self::Ptr>>(&self, own: &'a Own<T, Self::Ptr>) -> Ref<'a, T> {
        match own.raw.kind() {
            Kind::Offset(offset) => {
                /*
                let offset = usize::try_from(offset.get()).unwrap();
                let range = offset .. offset + T::blob_layout(own.metadata()).size();
                let words = self.words().get(range.clone())
                                        .unwrap_or_else(|| panic!("{:?}", range));

                let blob = Blob::<T, OffsetMut<'p>>::new(words, own.metadata()).unwrap();

                let blob = unsafe { blob.assume_fully_valid() };

                T::load_blob(blob, self)
                */
                todo!()
            },
            Kind::Ptr(ptr) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(ptr.cast().as_ptr(), own.metadata)
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T: ?Sized + Load<Self::Ptr>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
        let ptr = ptr.into_inner().into_inner();

        match unsafe { ptr.raw.try_take::<T>(ptr.metadata) } {
            Ok(owned) => owned,
            Err(offset) => {
                /*
                let offset = usize::try_from(offset.get()).unwrap();
                let range = offset .. offset + T::blob_layout(metadata).size();
                let words = self.words().get(range.clone())
                                        .unwrap_or_else(|| panic!("{:?}", range));

                let blob = Blob::<T, Self>::new(words, metadata).unwrap();

                let blob = unsafe { blob.assume_fully_valid() };

                T::decode_blob(blob, self)
                */
                todo!()
            },
        }
    }
}

#[cfg(test)]
mod test {
}
