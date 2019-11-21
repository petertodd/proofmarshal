use std::convert::TryInto;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use std::slice;
use std::ops::{self, Range};
use std::sync::Arc;

use memmap::Mmap;

use owned::{Ref, Take};

use crate::{
    Alloc,
    Get,
    FatPtr,
    Zone,
    own::Own,
    bag::Bag,
    pile::*,
    pointee::Pointee,
    never::NeverAllocator,
    marshal::{*, blob::*},
};

pub mod disk;
use self::disk::*;

#[derive(Debug)]
pub struct Hoard<'h, V = ()> {
    marker: PhantomData<fn(V)>,
    anchor: &'h mut (),
    fd: File,
    snapshot: Snapshot<'h>,
}

#[derive(Debug)]
pub struct HoardMut<'h, V = ()>(Hoard<'h, V>);

impl<V: Flavor> Hoard<'static, V> {
    pub fn open<F, R>(path: impl AsRef<Path>, f: F) -> io::Result<R>
        where F: for<'h> FnOnce(Hoard<'h, V>) -> R
    {
        let fd = OpenOptions::new()
                    .read(true)
                    .open(path)?;

        Self::open_fd(fd, f)
    }

    pub fn open_fd<F, R>(mut fd: File, f: F) -> io::Result<R>
        where F: for<'h> FnOnce(Hoard<'h, V>) -> R
    {
        fd.seek(SeekFrom::Start(0))?;
        let header = FileHeader::<V>::read(&mut fd)?;

        // TODO: where should we validate header version etc?

        fd.seek(SeekFrom::End(0))?;

        let mut anchor = ();
        unsafe {
            let this = Hoard::new_unchecked(fd, &mut anchor)?;
            Ok(f(this))
        }
    }
}

impl<'h, V> Hoard<'h, V> {
    unsafe fn new_unchecked(fd: File, anchor: &'h mut ()) -> io::Result<Self> {
        Ok(Self {
            marker: PhantomData,
            anchor,
            snapshot: Snapshot::new(Mapping::from_file(&fd)?),
            fd,
        })
    }

    pub fn roots<T>(&self) -> IterRoots<'h, T> {
        IterRoots::new(self.snapshot.clone())
    }
}

#[derive(Debug)]
pub struct Root<'h, T> {
    marker: PhantomData<fn() -> T>,
    snapshot: Snapshot<'h>,
}

impl<'h, T> Root<'h, T> {
    fn new(snapshot: Snapshot<'h>) -> Self {
        Self { marker: PhantomData, snapshot }
    }
}

#[derive(Debug)]
pub enum ValidateRootError<E> {
    Offset,
    Root(E),
}

impl<'h, T> Root<'h, T> {
    pub fn validate<'a>(&'a self) -> Result<Ref<'a, T>, ValidateRootError<T::Error>>
        where T: Decode<Offset<'static, 'h>>
    {
        let offset = self.snapshot.mapping.len()
                         .checked_sub(T::BLOB_LAYOUT.size())
                         .map(|offset| Offset::new(offset).unwrap())
                         .ok_or(ValidateRootError::Offset)?;

        let pile = self.snapshot.pile();

        todo!()
    }
}

#[derive(Debug)]
pub struct RootMut<'h, T>(Root<'h, T>);

#[derive(Debug, Clone)]
pub struct IterRoots<'h, T> {
    marker: PhantomData<fn() -> T>,
    snapshot: Snapshot<'h>,
    idx_front: usize,
    idx_back: usize,
}

#[derive(Debug, Clone)]
pub struct IterRootsMut<'h, T>(IterRoots<'h,T>);


impl<'h, T> IterRoots<'h, T> {
    pub fn new(snapshot: Snapshot<'h>) -> Self {
        let marks = snapshot.mapping.as_marks();
        Self {
            marker: PhantomData,
            idx_front: 0,
            idx_back: marks.len().saturating_sub(1),
            snapshot,
        }
    }
}

impl<'h, T> Iterator for IterRoots<'h, T> {
    type Item = Root<'h, T>;

    fn next(&mut self) -> Option<Root<'h, T>> {
        debug_assert!(self.idx_front <= self.idx_back);

        if self.idx_front == self.idx_back {
            None
        } else if self.snapshot.mapping.as_marks()[self.idx_front].is_valid(self.idx_front) {
            let mut root_snap = self.snapshot.clone();
            root_snap.truncate(self.idx_front * mem::size_of::<Mark>());

            self.idx_front += 1;
            Some(Root::new(root_snap))
        } else {
            None
        }
    }
}

impl<'h, T> DoubleEndedIterator for IterRoots<'h, T> {
    fn next_back(&mut self) -> Option<Root<'h, T>> {
        debug_assert!(self.idx_front <= self.idx_back);
        if self.idx_front == self.idx_back {
            None
        } else if self.snapshot.mapping.as_marks()[self.idx_back].is_valid(self.idx_back) {
            let mut root_snap = self.snapshot.clone();
            root_snap.truncate(self.idx_back * mem::size_of::<Mark>());

            self.idx_back -= 1;
            Some(Root::new(root_snap))
        } else {
            None
        }
    }
}

impl<'h, T> Iterator for IterRootsMut<'h, T> {
    type Item = RootMut<'h, T>;

    fn next(&mut self) -> Option<RootMut<'h, T>> {
        self.0.next().map(|root| RootMut(root))
    }
}

impl<'h, T> DoubleEndedIterator for IterRootsMut<'h, T> {
    fn next_back(&mut self) -> Option<RootMut<'h, T>> {
        self.0.next_back().map(|root| RootMut(root))
    }
}

impl<V: Flavor> HoardMut<'static, V> {
    pub fn create<F, R>(path: impl AsRef<Path>, f: F) -> io::Result<R>
        where F: for<'h> FnOnce(HoardMut<'h, V>) -> R
    {
        let mut fd = OpenOptions::new()
                        .read(true)
                        .append(true)
                        .create_new(true)
                        .open(path)?;

        let header = FileHeader::<V>::default();

        fd.write_all(header.as_bytes())?;

        Self::open_fd(fd, f)
    }

    pub fn open<F, R>(path: impl AsRef<Path>, f: F) -> io::Result<R>
        where F: for<'h> FnOnce(HoardMut<'h, V>) -> R
    {
        let fd = OpenOptions::new()
                    .read(true)
                    .append(true)
                    .open(path)?;

        Self::open_fd(fd, f)
    }

    pub fn open_fd<F, R>(mut fd: File, f: F) -> io::Result<R>
        where F: for<'h> FnOnce(HoardMut<'h, V>) -> R
    {
        fd.seek(SeekFrom::Start(0))?;
        let header = FileHeader::<V>::read(&mut fd)?;

        // TODO: where should we validate header version etc?

        fd.seek(SeekFrom::End(0))?;

        let mut anchor = ();
        unsafe {
            let this = HoardMut(Hoard::new_unchecked(fd, &mut anchor)?);
            Ok(f(this))
        }
    }
}

impl<'h, V> HoardMut<'h, V> {
    pub fn roots_mut<T>(&self) -> IterRootsMut<'h, T> {
        IterRootsMut(self.roots())
    }

    pub fn push_root<T>(&mut self, root: T) {
        todo!()
    }
}

impl<'h, V> ops::Deref for HoardMut<'h, V> {
    type Target = Hoard<'h,V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


#[derive(Debug, Clone)]
pub struct Snapshot<'h> {
    marker: PhantomData<&'h mut ()>,
    mapping: Mapping,
}

impl<'h> Snapshot<'h> {
    unsafe fn new(mapping: Mapping) -> Self {
        Self { marker: PhantomData, mapping, }
    }

    fn truncate(&mut self, len: usize) {
        self.mapping.truncate(len)
    }

    fn pile<'s>(&'s self) -> Pile<'s, 'h> {
        unsafe {
            Pile::new_unchecked(&self.mapping.slice())
        }
    }
}

/*
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

#[derive(Debug)]
pub enum ValidatePtrError {
    Ptr {
        offset: Offset<'static, 'static>,
        size: usize,
    },
    Value(Box<dyn crate::marshal::Error>),
}

impl<'s,'f> ValidatePtr<Offset<'s,'f>> for &'s Snapshot<'f> {
    type Error = ValidatePtrError;

    fn validate_ptr<'p, T: ?Sized + Load<Offset<'s,'f>>>(&mut self, ptr: &'p FatPtr<T,Offset<'s,'f>>)
        -> Result<BlobValidator<'p, T, Offset<'s, 'f>>, Self::Error>
    {
        let blob = self.try_get_blob(ptr)?;
        match T::validate_blob(blob) {
            Err(e) => Err(ValidatePtrError::Value(Box::new(e))),
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
*/

#[derive(Debug)]
pub struct SnapshotMut<'f>(Snapshot<'f>);

impl<'f> From<Snapshot<'f>> for SnapshotMut<'f> {
    fn from(snapshot: Snapshot<'f>) -> Self {
        Self(snapshot)
    }
}

/*

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
*/
