//! In-place persistence.

#![feature(never_type)]
#![feature(associated_type_bounds)]

use core::task::Poll;
use core::borrow::Borrow;
use core::mem::{ManuallyDrop, MaybeUninit};

use pointee::Pointee;

mod blob;
pub use self::blob::*;

pub mod impls;
pub mod own;
use self::own::Own;

mod refs;
pub use self::refs::*;

pub mod pile;

pub trait Zone : Sized {
    type Ptr;
    type PersistPtr : Encode<Self> + Copy;

    type Allocator;
}

impl Zone for ! {
    type Ptr = !;
    type PersistPtr = !;
    type Allocator = !;
}

/// A *value* that can be saved in a zone.
pub trait Save<Z: Zone> : Pointee<Metadata: Encode<Z>> {
    type Owned : Borrow<Self>;

    unsafe fn to_owned(this: &mut ManuallyDrop<Self>) -> Self::Owned;

    type Save : SavePoll<Zone = Z, Target = Self, PersistMetadata = Self::Metadata>;
    fn save(owned: Self::Owned) -> Self::Save;
}

pub trait SavePoll : Sized {
    type Zone : Zone;
    type Target : ?Sized + Pointee;
    type PersistMetadata : Encode<Self::Zone>;

    fn poll<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(<Self::Zone as Zone>::PersistPtr, Self::PersistMetadata), P::Error>>
        where P: Saver<Zone = Self::Zone>;
}

pub trait Encode<Z: Zone> : Sized {
    const BLOB_LAYOUT: BlobLayout;

    type Encode : EncodePoll<Zone = Z, Target=Self>;
    fn encode(self) -> Self::Encode;
}

pub trait EncodePoll {
    type Zone : Zone;
    type Target : Encode<Self::Zone>;

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: Saver<Zone = Self::Zone>
    {
        let _ = saver;
        Ok(()).into()
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error>;
}

pub trait Saver {
    type Zone : Zone;
    type Error;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [MaybeUninit<u8>]))
        -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>;

    fn save_own<T: ?Sized + Pointee>(&mut self, own: Own<T, Self::Zone>) -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>
        where T: Save<Self::Zone>;
}

impl<Z: Zone, T: Encode<Z>> Save<Z> for T {
    type Owned = T;

    unsafe fn to_owned(this: &mut ManuallyDrop<Self>) -> Self::Owned {
        (this as *const _ as *const Self).read()
    }

    type Save = SaveValue<T::Encode>;
    fn save(owned: Self) -> Self::Save {
        SaveValue {
            encoder: owned.encode(),
        }
    }
}

#[derive(Debug)]
pub struct SaveValue<E> {
    encoder: E,
}

impl<E: EncodePoll> SavePoll for SaveValue<E> {
    type Zone = E::Zone;
    type Target = E::Target;
    type PersistMetadata = ();

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(<Self::Zone as Zone>::PersistPtr, ()), P::Error>>
        where P: Saver<Zone = Self::Zone>
    {
        match self.encoder.poll(saver)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                let ptr = saver.save_blob(E::Target::BLOB_LAYOUT.size(), |dst| {
                    self.encoder.encode_blob(dst)
                                .unwrap_or_else(|never| never)
                })?;
                Ok((ptr, ())).into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let e = Encode::<!>::encode((42u8, true));

        let mut dst = [0u8; 2];
        e.encode_blob(&mut dst[..]).unwrap();

        assert_eq!(dst, [42, 1]);
    }
}
