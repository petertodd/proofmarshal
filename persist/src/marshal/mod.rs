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
}

pub trait PtrSaver {
    type Zone : Zone;
    type Error;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [u8]))
        -> Result<<Self::Zone as Zone>::PersistPtr, Self::Error>;

    fn save_own<T: ?Sized + Save<Self::Zone>>(&mut self, own: Own<T, Self::Zone>)
        -> Result<<Self::Zone as Zone>::PersistPtr, T::SavePoll>;
}

impl PtrSaver for () {
    type Zone = !;
    type Error = !;

    fn save_blob(&mut self, _: usize, _: impl FnOnce(&mut [u8]))
        -> Result<!, Self::Error>
    {
        panic!()
    }

    fn save_own<T: ?Sized + Save<Self::Zone>>(&mut self, own: Own<T, Self::Zone>)
        -> Result<!, T::SavePoll>
    {
        match *own.ptr() {}
    }
}

pub trait ValidateChildren<Z: Zone> {
    fn validate_children<V>(&mut self, ptr_validator: V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>;
}

impl<Z: Zone> ValidateChildren<Z> for () {
    fn validate_children<V>(&mut self, _ptr_validator: V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        Ok(()).into()
    }
}

pub trait Load<Z: Zone> : Save<Z> {
    type Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }
}

pub trait Loader<Z: Zone> {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: Z::PersistPtr, metadata: T::Metadata) -> Own<T,Z>;

    fn allocator(&self) -> Z::Allocator;
}

impl Loader<!> for () {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: !, _: T::Metadata) -> Own<T,!> {
        match persist_ptr {}
    }

    fn allocator(&self) -> crate::never::NeverAllocator<!> {
        panic!()
    }
}

impl<Z: Zone, L: Loader<Z>> Loader<Z> for &'_ L {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: Z::PersistPtr, metadata: T::Metadata) -> Own<T,Z> {
        (&**self).load_ptr(persist_ptr, metadata)
    }

    fn allocator(&self) -> Z::Allocator {
        (&**self).allocator()
    }
}

pub trait ValidatePtr<Z: Zone> {
    type Error;

    //fn validate_ptr<T: ?Sized + Load<Self::Zone>>(&mut self, persist_ptr: Z::PersistPtr, metadata: T::Metadata)
}

impl ValidatePtr<!> for () {
    type Error = !;
}

impl<'a, Z: Zone, T: ValidatePtr<Z>> ValidatePtr<Z> for &'a mut T {
    type Error = T::Error;
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
    const BLOB_LAYOUT: BlobLayout = <Z::PersistPtr as Save<!>>::BLOB_LAYOUT;

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

pub enum ValidateOwnError<Z: Zone> {
    Ptr(<Z::PersistPtr as Load<!>>::Error),
    Metadata,
}

pub enum ValidateOwn<T: ?Sized + Load<Z>, Z: Zone> {
    Own {
        ptr: Z::PersistPtr,
        metadata: T::Metadata,
    },
    Value(T::ValidateChildren),
}

impl<T: ?Sized + Pointee, Z: Zone> Load<Z> for Own<T,Z>
where T: Load<Z>
{
    type Error = ValidateOwnError<Z>;

    type ValidateChildren = ValidateOwn<T,Z>;

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        /*
        let mut v = blob.validate();
        let ptr = v.field::<Z::PersistPtr>().map_err(|e| ValidateOwnError::Ptr(e))?;
        let metadata = v.field::<Z::PersistPtr>().map_err(|e| ValidateOwnError::Ptr(e))?;
        */
        todo!()
    }

    /*
    unsafe fn validate_children<'p, V: ValidatePtr<Zone = Z>>(
            state: &mut Self::State,
            blob: ValidBlob<'p, Self, Z>,
            ptr_verifier: V,
        ) -> Poll<Result<FullyValidBlob<'p, Self, Z>, V::Error>>
    {
        match state {
            ValidateOwn::Own { ptr, metadata } => {
                todo!()
            },
            ValidateOwn::Value(state) => {
                todo!()
            },
        }
    }
    */

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        todo!()
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self {
        todo!()
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> ValidateChildren<Z> for ValidateOwn<T,Z> {
    fn validate_children<V>(&mut self, ptr_validator: V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        match self {
            ValidateOwn::Own { ptr, metadata } => {
                todo!()
            },
            ValidateOwn::Value(v) => v.validate_children(ptr_validator),
        }
    }
}

pub fn encode<T: Save<!>>(value: T) -> Vec<u8> {
    let mut saver = T::save_poll(value);

    match saver.save_children(&mut ()) {
        Poll::Ready(Ok(())) => {},
        x => panic!(),
    }

    let mut dst = vec![0; T::BLOB_LAYOUT.size()];
    saver.encode_blob(&mut dst[..]).unwrap();
    dst
}

pub fn try_decode<'a, T: ?Sized + Load<!>>(blob: Blob<'a, T,!>) -> Result<Ref<'a, T>, T::Error> {
    let mut validator = T::validate_blob(blob)?;

    match validator.poll(()) {
        Poll::Ready(Ok(fully_valid_blob)) => Ok(T::load_blob(fully_valid_blob, &mut ())),
        _ => panic!(),
    }
}

/*
pub fn test_try_decode<'a>(blob: Blob<'a, u64, !>)
    -> Result<Ref<'a, u64>, !>
{
    try_decode(blob)
}
*/
