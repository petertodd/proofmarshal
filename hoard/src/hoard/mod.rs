use std::io;
use std::marker::PhantomData;
use std::mem;
use std::slice;
use std::sync::Arc;

use memmap::Mmap;

pub mod disk;
use self::disk::*;
pub use self::disk::HoardFile;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;

    use tempfile::tempfile;

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
