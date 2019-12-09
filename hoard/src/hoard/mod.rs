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

use singlelife::Unique;

use crate::{
    FatPtr, ValidPtr,
    Alloc, Zone,
    pointee::Pointee,
    marshal::{Load, Decode, Dumper, Encode},
    pile::{
        Pile, PileMut, Offset, OffsetMut, Snapshot, Mapping,
        offset::Kind,
    },
};

pub mod disk;
use self::disk::*;

unsafe impl Mapping for Mmap {
    fn as_bytes(&self) -> &[u8] {
        &self[..]
    }
}

#[derive(Debug)]
pub struct Hoard<V = ()> {
    marker: PhantomData<fn(V)>,
    fd: File,
    mapping: Arc<Mmap>,
}

#[derive(Debug)]
pub struct HoardMut<V = ()>(Hoard<V>);

impl<V: Flavor> Hoard<V> {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let fd = OpenOptions::new()
                    .read(true)
                    .open(path)?;

        Self::open_fd(fd)
    }

    pub fn open_fd(mut fd: File) -> io::Result<Self> {
        fd.seek(SeekFrom::Start(0))?;
        let header = FileHeader::<V>::read(&mut fd)?;

        // TODO: where should we validate header version etc?

        fd.seek(SeekFrom::End(0))?;

        let mapping = unsafe { Mmap::map(&fd)? };

        Ok(Self {
            marker: PhantomData,
            mapping: Arc::new(mapping),
            fd,
        })
    }

    pub fn snapshot<'h>(self: &'h Unique<Self>) -> Snapshot<'h, Arc<Mmap>> {
        unsafe {
            Snapshot::new_unchecked_with_range(
                self.mapping.clone(),
                mem::size_of::<FileHeader>() ..
            ).expect("mapping to have file header")
        }
    }

    pub fn roots<'h, T>(self: &'h Unique<Self>) -> IterRoots<'h, T>
        where T: Decode<Pile<'static, 'h>>
    {
        IterRoots::new(self.snapshot())
    }
}

#[derive(Debug)]
pub struct Root<'h, T> {
    marker: PhantomData<fn() -> T>,
    snapshot: Snapshot<'h, Arc<Mmap>>,
}

impl<'h, T> Root<'h, T> {
    fn new(snapshot: Snapshot<'h, Arc<Mmap>>) -> Self {
        Self { marker: PhantomData, snapshot }
    }

    pub fn offset<'s>(&'s self) -> FatPtr<T, Offset<'s, 'h>>
        where T: Decode<Pile<'s, 'h>>
    {
        let padding = align_offset(T::BLOB_LAYOUT.size() as u64, mem::size_of::<Mark>());
        let offset = self.snapshot.len()
                         .saturating_sub(T::BLOB_LAYOUT.size() + padding);
        let offset = Offset::new(&self.snapshot, offset, T::BLOB_LAYOUT.size())
                            .expect("undersized snapshot");
        FatPtr {
            raw: offset,
            metadata: (),
        }
    }

    pub fn fully_validate<'s>(&'s self) -> Result<T, T::Error>
        where T: Decode<Pile<'s, 'h>>
    {
        let mut pile = Pile::new(&self.snapshot);

        let root = self.offset();
        let blob = pile.get_blob(&root).unwrap();

        let mut validator = T::validate_blob(blob)?;

        let fully_valid_blob = validator.poll(&mut pile).expect("FIXME");

        Ok(T::decode_blob(fully_valid_blob, &pile))
    }
}

#[derive(Debug)]
pub struct RootMut<'h, T>(Root<'h, T>);

impl<'h, T> RootMut<'h, T> {
    fn new(snapshot: Snapshot<'h, Arc<Mmap>>) -> Self {
         Self(Root::new(snapshot))
    }

    pub fn offset<'s>(&'s self) -> FatPtr<T, Offset<'s, 'h>>
        where T: Decode<PileMut<'s, 'h>>
    {
        let padding = align_offset(T::BLOB_LAYOUT.size() as u64, mem::size_of::<Mark>());
        let offset = self.0.snapshot.len()
                         .saturating_sub(T::BLOB_LAYOUT.size() + padding);
        let offset = Offset::new(&self.0.snapshot, offset, T::BLOB_LAYOUT.size())
                            .expect("undersized snapshot");
        FatPtr {
            raw: offset,
            metadata: (),
        }
    }

    pub fn fully_validate<'s>(&'s self) -> Result<T, T::Error>
        where T: Decode<PileMut<'s, 'h>>
    {
        let mut pile = PileMut::from(Pile::new(&self.0.snapshot));

        let root = self.offset();
        let blob = pile.get_blob(&root).unwrap();

        let mut validator = T::validate_blob(blob)?;

        let fully_valid_blob = validator.poll(&mut pile).expect("FIXME");

        Ok(T::decode_blob(fully_valid_blob, &pile))
    }
}

#[derive(Debug, Clone)]
pub struct IterRoots<'h, T> {
    marker: PhantomData<fn() -> T>,
    snapshot: Snapshot<'h, Arc<Mmap>>,
    idx_front: usize,
    idx_back: usize,
}

#[derive(Debug, Clone)]
pub struct IterRootsMut<'h, T>(IterRoots<'h,T>);

impl<'h, T> IterRoots<'h, T>
where T: Decode<Pile<'static, 'h>>
{
    fn new(snapshot: Snapshot<'h, Arc<Mmap>>) -> Self {
        let marks = Mark::as_marks(&snapshot);

        Self {
            marker: PhantomData,
            idx_front: (T::BLOB_LAYOUT.size() + mem::size_of::<Mark>() - 1) / mem::size_of::<Mark>(),
            idx_back: marks.len(),
            snapshot,
        }
    }
}

impl<'h, T> IterRootsMut<'h, T>
where T: Decode<PileMut<'static, 'h>>
{
    fn new(snapshot: Snapshot<'h, Arc<Mmap>>) -> Self {
        let marks = Mark::as_marks(&snapshot);

        Self(IterRoots {
            marker: PhantomData,
            idx_front: (T::BLOB_LAYOUT.size() + mem::size_of::<Mark>() - 1) / mem::size_of::<Mark>(),
            idx_back: marks.len(),
            snapshot,
        })
    }
}

impl<'h, T> Iterator for IterRoots<'h, T>
where T: Decode<Pile<'static, 'h>>
{
    type Item = Root<'h, T>;

    fn next(&mut self) -> Option<Root<'h, T>> {
        while self.idx_front < self.idx_back {
            let idx = self.idx_front;
            self.idx_front += 1;

            let marks = Mark::as_marks(&self.snapshot);
            if marks[idx].is_valid(idx.try_into().unwrap()) {
                let mut root_snap = self.snapshot.clone();
                root_snap.truncate(idx * mem::size_of::<Mark>());

                return Some(Root::new(root_snap))
            }
        }
        None
    }
}

impl<'h, T> DoubleEndedIterator for IterRoots<'h, T>
where T: Decode<Pile<'static, 'h>>
{
    fn next_back(&mut self) -> Option<Root<'h, T>> {
        while self.idx_front < self.idx_back {
            self.idx_back -= 1;
            let idx = self.idx_back;

            let marks = Mark::as_marks(&self.snapshot);
            if marks[idx].is_valid(idx.try_into().unwrap()) {
                let mut root_snap = self.snapshot.clone();
                root_snap.truncate(idx * mem::size_of::<Mark>());

                return Some(Root::new(root_snap))
            }
        }
        None
    }
}

impl<'h, T> Iterator for IterRootsMut<'h, T>
where T: Decode<PileMut<'static, 'h>>
{
    type Item = RootMut<'h, T>;

    fn next(&mut self) -> Option<RootMut<'h, T>> {
        while self.0.idx_front < self.0.idx_back {
            let idx = self.0.idx_front;
            self.0.idx_front += 1;

            let marks = Mark::as_marks(&self.0.snapshot);
            if marks[idx].is_valid(idx.try_into().unwrap()) {
                let mut root_snap = self.0.snapshot.clone();
                root_snap.truncate(idx * mem::size_of::<Mark>());

                return Some(RootMut::new(root_snap))
            }
        }
        None
    }
}

impl<'h, T> DoubleEndedIterator for IterRootsMut<'h, T>
where T: Decode<PileMut<'static, 'h>>
{
    fn next_back(&mut self) -> Option<RootMut<'h, T>> {
        while self.0.idx_front < self.0.idx_back {
            self.0.idx_back -= 1;
            let idx = self.0.idx_back;

            let marks = Mark::as_marks(&self.0.snapshot);
            if marks[idx].is_valid(idx.try_into().unwrap()) {
                let mut root_snap = self.0.snapshot.clone();
                root_snap.truncate(idx * mem::size_of::<Mark>());

                return Some(RootMut::new(root_snap))
            }
        }
        None
    }
}

impl<V: Flavor> HoardMut<V> {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let fd = OpenOptions::new()
                    .read(true)
                    .append(true)
                    .open(path)?;

        Self::open_fd(fd)
    }

    pub fn open_fd(fd: File) -> io::Result<Self> {
        Ok(Self(Hoard::open_fd(fd)?))
    }

    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut fd = OpenOptions::new()
                        .read(true)
                        .append(true)
                        .create_new(true)
                        .open(path)?;

        let header = FileHeader::<V>::default();

        fd.write_all(header.as_bytes())?;

        Self::open_fd(fd)
    }


    pub fn roots<'h, T>(self: &'h Unique<Self>) -> IterRootsMut<'h, T>
        where T: Decode<PileMut<'static, 'h>>
    {
        IterRootsMut::new(self.as_hoard().snapshot())
    }

    pub fn push_root<'s, 'h, T>(self: &'h mut Unique<Self>, root: T) -> io::Result<u64>
        where T: Encode<PileMut<'s, 'h>>
    {
        // HACK: We need to borrow the fd again later to rebuild the mapping, which the borrow
        // checker isn't happy with.
        let fd = unsafe { &mut *(&mut self.0.fd as *mut _) };
        let mut dumper = BlobDumper::new(fd)?;

        let mut state = root.init_encode_state();
        root.encode_poll(&mut state, &mut dumper)?;

        let root_offset = dumper.commit_root_with(
            T::BLOB_LAYOUT.size(),
            | dst | {
                match root.encode_blob(&state, dst) {
                    Ok(()) => (),
                    Err(never) => never,
                }
            })?;

        unsafe {
            self.0.mapping = Arc::new(Mmap::map(&self.0.fd)?);
        }

        Ok(root_offset)
    }

    pub fn as_hoard<'h>(self: &'h Unique<Self>) -> &'h Unique<Hoard<V>> {
        // Safe because we're a #[repr(transparent)] wrapper.
        unsafe {
            &*(self as *const _ as *const _)
        }
    }
}

impl<'s, 'h> Dumper<PileMut<'s, 'h>> for &'_ mut BlobDumper<'h> {
    type Pending = io::Error;

    fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, OffsetMut<'s, 'h>>) -> Result<Offset<'s, 'h>, &'p T> {
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

    fn try_save_blob(self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s, 'h>), io::Error> {
        let offset = self.write_blob_with(size, f)?;

        // FIXME: what exactly is the safety story here?
        let offset = unsafe { Offset::new_unchecked(offset as usize) };

        Ok((self, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;
    use tempfile::tempdir;

    use crate::OwnedPtr;

    #[test]
    fn hoardmut_push_root() -> io::Result<()> {
        let tmpdir = tempdir()?;

        let hoard = HoardMut::<()>::create(
            tmpdir.path().join("hoardmut")
        )?;

        Unique::new(hoard, |hoard| {
            let mut alloc = PileMut::allocator();
            let owned = alloc.alloc(42u8);

            assert_eq!(hoard.push_root(owned)?, 16);

            assert_eq!(&hoard.0.mapping[..],
                &[0, 72, 111, 97, 114, 100, 32, 70, 105, 108, 101,  0,  0,  0,  0,  0,
                 76, 76,  76, 76,  76,  76, 76, 76,  76,  76,  76, 76, 76, 76, 76, 76,
                 42,
                      0, 0, 0, 0, 0, 0, 0,
                  1, 0, 0, 0, 0, 0, 0, 0,
                  253, 255, 255, 255, 255, 255, 255, 255][..]);

            let root = hoard.roots::<OwnedPtr<u8,OffsetMut>>()
                            .last().unwrap();
            let root = root.fully_validate().unwrap();

            if let Kind::Offset(offset) = root.raw.kind() {
                assert_eq!(offset.get(), 0);
            } else {
                panic!()
            }

            let owned = alloc.alloc((root, 42u8));
            //assert_eq!(hoard.push_root(owned)?, 16);

            Ok(())
        })
    }

    #[test]
    fn hoardmut_push_root_tuple() -> io::Result<()> {
        let tmpdir = tempdir()?;

        let hoard = HoardMut::<()>::create(
            tmpdir.path().join("hoardmut")
        )?;

        Unique::new(hoard, |hoard| {
            let v = (8u8, 16u16, 32u32);
            assert_eq!(hoard.push_root(v)?, 8);

            for root in hoard.as_hoard().roots::<(u8, u16, u32)>() {
                let root = root.fully_validate().unwrap();
                assert_eq!(root, (8, 16, 32));
            }

            Ok(())
        })
    }

    #[test]
    fn hoardmut_push_root_primitive() -> io::Result<()> {
        let tmpdir = tempdir()?;

        let hoard = HoardMut::<()>::create(
            tmpdir.path().join("hoardmut")
        )?;

        Unique::new(hoard, |hoard| {
            assert_eq!(hoard.push_root(0u8)?, 8);
            assert_eq!(hoard.push_root(1u8)?, 24);
            assert_eq!(hoard.push_root(2u8)?, 40);

            for (i, root) in hoard.as_hoard().roots::<u8>().enumerate() {
                let root = root.fully_validate().unwrap();
                assert_eq!(i, root as usize);
            }

            Ok(())
        })
    }

    /*
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
*/
}
