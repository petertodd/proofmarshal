use super::*;

use core::mem::{self, MaybeUninit};
use core::slice;

pub mod impls;

pub mod blob;
use self::blob::*;

/// A *value* that can be saved in a zone.
pub trait Save<Z: Zone> : Owned + Pointee {
    const BLOB_LAYOUT: BlobLayout;

    #[inline(always)]
    fn blob_layout(metadata: Self::Metadata) -> BlobLayout {
        assert_eq!(mem::size_of_val(&metadata), 0);

        Self::BLOB_LAYOUT
    }

    type SavePoll : SavePoll<Zone = Z, Target = Self>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll;
}

pub trait Load<Z: Zone> : Owned + Pointee {
}


pub trait SavePoll : Sized {
    type Zone : Zone;
    type Target : ?Sized + Save<Self::Zone>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: PtrSaver<Zone = Self::Zone>
    {
        Poll::Ready(Ok(()))
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error>;

    fn metadata(&self) -> <Self::Target as Pointee>::Metadata {
        if mem::size_of::<<Self::Target as Pointee>::Metadata>() == 0 {
            unsafe { MaybeUninit::uninit().assume_init() }
        } else {
            unimplemented!()
        }
    }

    //fn save_blob<P>(&mut self, ptr_saver: &mut P) -> Result<(<Self::Zone as Zone>::PersistPtr, Self::PersistMetadata), P::Error>
    //    where P: Saver<Zone = Self::Zone>;
}

pub trait EncodePoll {
    type Zone : Zone;
    type Target : ?Sized + Pointee;

    /*
    type Zone : Zone;
    type Target : Encode<Self::Zone>;

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: Saver<Zone = Self::Zone>
    {
        let _ = saver;
        Ok(()).into()
    }

    */
}

pub trait PtrSaver {
    type Zone : Zone;
    type Error;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [u8]))
        -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>;

    fn save_own<T: ?Sized + Save<Self::Zone>>(&mut self, own: Own<T, Self::Zone>)
        -> Result<<Self::Zone as Zone>::PersistPtr, T::SavePoll>;
}

#[derive(Debug)]
pub enum SaveOwnPoll<T: ?Sized + Save<Z>, Z: Zone> {
    Own(Own<T,Z>),
    Pending(T::SavePoll),
    Done {
        persist_ptr: Z::PersistPtr,
        metadata: T::Metadata,
    },
    Poisoned,
}

impl<T: ?Sized + Pointee, Z: Zone> Save<Z> for Own<T,Z>
where T: Save<Z>
{
    const BLOB_LAYOUT: BlobLayout = <Z::PersistPtr as Save<Z>>::BLOB_LAYOUT;

    type SavePoll = SaveOwnPoll<T, Z>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        SaveOwnPoll::Own(this.take_sized())
    }
}

impl<T: ?Sized + Pointee, Z: Zone> SavePoll for SaveOwnPoll<T, Z>
where T: Save<Z>
{
    type Zone = Z;
    type Target = Own<T,Z>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: PtrSaver<Zone = Self::Zone>
    {
        match self {
            Self::Done { .. } => Ok(()).into(),
            Self::Poisoned => panic!(),

            Self::Pending(pending) =>
                match pending.save_children(ptr_saver)? {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(()) => {
                        let metadata = pending.metadata();
                        let size = T::blob_layout(metadata).size();
                        let persist_ptr = ptr_saver.save_blob(size, |dst| {
                            pending.encode_blob(dst).unwrap()
                        })?;

                        *self = Self::Done { persist_ptr, metadata };

                        Ok(()).into()
                    },
                },
            Self::Own(_) => {
                if let Self::Own(own) = mem::replace(self, Self::Poisoned) {
                    let metadata = own.metadata();
                    match ptr_saver.save_own(own) {
                        Ok(persist_ptr) => {
                            *self = Self::Done { persist_ptr, metadata };
                            Ok(()).into()
                        },
                        Err(pending) => {
                            *self = Self::Pending(pending);
                            self.save_children(ptr_saver)
                        },
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        if let Self::Done { persist_ptr, metadata } = self {
            unsafe fn as_bytes<T>(x: &T) -> &[u8] {
                slice::from_raw_parts(x as *const T as *const u8,
                                      mem::size_of::<T>())
            }

            unsafe {
                dst.write_bytes(as_bytes(persist_ptr))?
                   .write_bytes(as_bytes(metadata))?
                   .done()
            }
        } else {
            panic!()
        }
    }
}


/*
impl<Z: Zone, T: Encode<Z>> Save<Z> for T {
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
*/
