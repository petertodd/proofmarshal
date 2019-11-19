use std::convert::TryInto;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::slice;
use std::sync::Arc;

use memmap::Mmap;

use owned::{Ref, Take};

use crate::marshal::{
    Load, LoadPtr, ValidatePtr,
    blob::{
        Blob, BlobValidator,
        FullyValidBlob,
    }
};

use crate::{
    Alloc,
    Get,
    FatPtr,
    Zone,
    own::Own,
    pointee::Pointee,
    never::NeverAllocator,
};

pub mod disk;
use self::disk::*;
pub use self::disk::HoardFile;

mod offset;
pub use self::offset::*;


#[derive(Debug)]
pub struct Hoard<'f> {
    backend: &'f mut HoardFile,
}

impl<'f> Hoard<'f> {
    pub unsafe fn new_unchecked(backend: &'f mut HoardFile) -> Self {
        Self { backend }
    }

    pub fn snapshot(&self) -> Snapshot<'f> {
        unsafe {
            Snapshot::new(self.backend.mapping.clone())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot<'f> {
    marker: PhantomData<&'f mut ()>,
    mapping: Mapping,
}

impl<'f> Snapshot<'f> {
    unsafe fn new(mapping: Mapping) -> Self {
        Self { marker: PhantomData, mapping, }
    }

    pub fn roots(&self) -> impl DoubleEndedIterator<Item=Root<'f>> + '_ {
        let cloned = self.clone();
        self.mapping.mark_offsets()
            .map(move |offset| Root { snapshot: cloned.clone(), offset })
    }

    fn try_get_blob<'s, 'p, T: ?Sized + Load<Offset<'s,'f>>>(&'s self, ptr: &'p FatPtr<T, Offset<'s, 'f>>)
        -> Result<Blob<'p, T, Offset<'s, 'f>>, ValidatePtrError>
    {
        let size = T::blob_layout(ptr.metadata).size();
        let start = ptr.raw.get().try_into().unwrap();
        match self.mapping.get(start .. start + size) {
            Some(slice) => Ok(Blob::new(slice, ptr.metadata).unwrap()),
            None => Err(ValidatePtrError::Ptr {
                offset: ptr.raw.to_static(),
                size
            }),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidatePtrError {
    Ptr {
        offset: Offset<'static, 'static>,
        size: usize,
    },
    Value, // FIXME: should return the error
}

impl<'s,'f> ValidatePtr<Offset<'s,'f>> for &'s Snapshot<'f> {
    type Error = ValidatePtrError;

    fn validate_ptr<'p, T: ?Sized + Load<Offset<'s,'f>>>(&mut self, ptr: &'p FatPtr<T,Offset<'s,'f>>)
        -> Result<BlobValidator<'p, T, Offset<'s, 'f>>, Self::Error>
    {
        let blob = self.try_get_blob(ptr)?;
        match T::validate_blob(blob) {
            Err(_) => Err(ValidatePtrError::Value),
            Ok(validator) => Ok(validator),
        }
    }
}

impl<'s,'f> LoadPtr<Offset<'s,'f>> for &'s Snapshot<'f> {
    fn load_blob<'a, T: ?Sized + Load<Offset<'s,'f>>>(&self, ptr: &'a FatPtr<T, Offset<'s,'f>>)
        -> FullyValidBlob<'a, T, Offset<'s,'f>>
    {
        let blob = self.try_get_blob(ptr).expect("FIXME");

        // FIXME: maybe we need a ValidFatPtr?
        unsafe { blob.assume_fully_valid() }
    }
}

impl<'s,'f> Zone for &'s Snapshot<'f> {
    type Ptr = Offset<'s,'f>;
    type Allocator = NeverAllocator<Self>;

    fn allocator() -> Self::Allocator {
        unreachable!()
    }
}

impl<'s,'f> Get for &'s Snapshot<'f> {
    fn get<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: &'p Own<T, Self::Ptr>) -> Ref<'p, T> {
        let blob = self.try_get_blob(ptr).expect("FIXME");
        let blob = unsafe { blob.assume_fully_valid() };
        T::load_blob(blob, self)
    }

    fn take<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
        let blob = self.try_get_blob(&ptr).expect("FIXME");
        let blob = unsafe { blob.assume_fully_valid() };
        T::decode_blob(blob, self)
    }
}

#[derive(Debug)]
pub struct SnapshotMut<'f>(Snapshot<'f>);

impl<'f> From<Snapshot<'f>> for SnapshotMut<'f> {
    fn from(snapshot: Snapshot<'f>) -> Self {
        Self(snapshot)
    }
}

impl<'s,'f> ValidatePtr<OffsetMut<'s,'f>> for &'s SnapshotMut<'f> {
    type Error = ValidatePtrError;

    fn validate_ptr<'p, T: ?Sized + Load<OffsetMut<'s,'f>>>(&mut self, ptr: &'p FatPtr<T, OffsetMut<'s,'f>>)
        -> Result<BlobValidator<'p, T, OffsetMut<'s, 'f>>, Self::Error>
    {
        todo!()
    }
}

impl<'s,'f> LoadPtr<OffsetMut<'s,'f>> for &'s SnapshotMut<'f> {
    fn load_blob<'a, T: ?Sized + Load<OffsetMut<'s,'f>>>(&self, ptr: &'a FatPtr<T, OffsetMut<'s,'f>>)
        -> FullyValidBlob<'a, T, OffsetMut<'s,'f>>
    {
        todo!()
    }
}

impl<'s,'f> Zone for &'s SnapshotMut<'f> {
    type Ptr = OffsetMut<'s,'f>;
    type Allocator = Self;

    fn allocator() -> Self::Allocator {
        unreachable!()
    }
}

impl<'s,'f> Alloc for &'s SnapshotMut<'f> {
    type Zone = Self;
    type Ptr = OffsetMut<'s,'f>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> Own<T, Self::Ptr> {
        src.take_unsized(|src| {
            unsafe {
                Own::new_unchecked(
                    FatPtr {
                        metadata: T::metadata(src),
                        raw: OffsetMut::alloc(src),
                    }
                )
            }
        })
    }

    fn zone(&self) -> Self {
        todo!()
    }
}

impl<'s,'f> Get for &'s SnapshotMut<'f> {
    fn get<'p, T: ?Sized + Load<Self::Ptr>>(&self, ptr: &'p Own<T, Self::Ptr>) -> Ref<'p, T> {
        match ptr.raw.kind() {
            Kind::Offset(offset) => {
                todo!()
            },
            Kind::Ptr(nonnull) => {
                let r: &'p T = unsafe {
                    &*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata)
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T: ?Sized + Load<Self::Ptr>>(&self, ptr: Own<T, Self::Ptr>) -> T::Owned {
        let fatptr = ptr.into_inner();
        match unsafe { fatptr.raw.try_take::<T>(fatptr.metadata) } {
            Ok(owned) => owned,
            Err(offset) => {
                todo!()
            },
        }
    }
}


#[derive(Debug, Clone)]
pub struct Root<'f> {
    snapshot: Snapshot<'f>,
    offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;

    use tempfile::tempfile;

    #[test]
    fn snapshotmut_zone() {
        let snap = unsafe { SnapshotMut::from(Snapshot::new(Mapping::from_buf([]))) };

        let owned_u8 = (&snap).alloc(42u8);
        assert_eq!(*(&snap).get(&owned_u8), 42);
        assert_eq!((&snap).take(owned_u8), 42);
    }

    #[test]
    fn snapshot_validate_ptr() {
        let snap = unsafe { Snapshot::new(Mapping::from_buf([])) };

        let fatptr: FatPtr<(),_> = Offset::new(0).unwrap().into();
        let _ = (&snap).validate_ptr(&fatptr).unwrap();

        let fatptr: FatPtr<u8,_> = Offset::new(0).unwrap().into();
        let _ = (&snap).validate_ptr(&fatptr).unwrap_err();

        let snap = unsafe { Snapshot::new(Mapping::from_buf([1,2,3,4])) };
        let fatptr: FatPtr<u32,_> = Offset::new(0).unwrap().into();
        let _ = (&snap).validate_ptr(&fatptr).unwrap();
    }

    #[test]
    fn snapshot_zone() {
        let snap = unsafe { Snapshot::new(Mapping::from_buf([42])) };

        let fatptr: FatPtr<u8,_> = Offset::new(0).unwrap().into();
        let owned = unsafe { Own::new_unchecked(fatptr) };

        assert_eq!((&snap).take(owned), 42);
    }

    #[test]
    fn hoardfile() -> io::Result<()> {
        let mut hoardfile = HoardFile::create_from_fd(tempfile()?)?;

        hoardfile.enter(|hoard| {
            let snap1 = hoard.snapshot();
            assert_eq!(snap1.mapping.len(), 0);

            let mut tx = Tx::new(hoard.backend)?;

            assert_eq!(tx.write_blob(&[])?, 0);
            assert_eq!(tx.write_blob(&[])?, 0);

            assert_eq!(tx.write_blob(&[1])?, 0);
            assert_eq!(tx.write_blob(&[2])?, 8);
            assert_eq!(tx.write_blob(&[])?, 16);
            assert_eq!(tx.write_blob(&[])?, 16);

            tx.commit()?;

            let snap2 = hoard.snapshot();
            assert_eq!(snap2.mapping.len(), 24);
            assert_eq!(&snap2.mapping[..],
                       [1, 0,0,0,0,0,0,0,
                        2, 0,0,0,0,0,0,0,
                        0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

            assert_eq!(snap2.mapping.mark_offsets().collect::<Vec<usize>>(),
                       vec![16]);

            Ok(())
        })
    }
}
