use super::*;

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};

use crate::marshal::*;
use crate::marshal::blob::*;

/// An owned pointer to a value in a `Zone`.
#[repr(C)]
pub struct Own<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<P>,
    metadata: T::Metadata,
}

impl<T: ?Sized + Pointee, P: Ptr> Own<T,P> {
    pub unsafe fn from_raw_parts(ptr: P, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
            metadata,
        }
    }

    pub fn into_raw_parts(self) -> (P, T::Metadata) {
        let mut this = ManuallyDrop::new(self);
        let ptr = unsafe { (&mut *this.ptr as *mut P).read() };
        (ptr, this.metadata)
    }

    pub fn ptr(&self) -> &P {
        &self.ptr
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for Own<T,P> {
    fn drop(&mut self) {
        let this = unsafe { core::ptr::read(self) };
        P::dealloc_own(this)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for Own<T,P>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        P::fmt_debug_own(self, f)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for Own<T,P>
where P: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&*self.ptr, f)
    }
}


impl<T, P, Q> Encode<Q> for Own<T,P>
where Q: Ptr + Encode<Q>,
      P: Ptr + Encode<Q>,
      T: ?Sized + Save<Q> + Load<P>,
{
    const BLOB_LAYOUT: BlobLayout = Q::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type EncodePoll = OwnEncoder<T, P, Q>;

    fn encode_poll(self) -> Self::EncodePoll {
        OwnEncoder::Own(self)
    }
}

pub enum OwnEncoder<T: ?Sized + Save<Q>, P: Ptr, Q: Encode<Q>> {
    Own(Own<T,P>),
    SaveValue(<T as Save<Q>>::SavePoll),
    Done {
        ptr_encoder: Q::EncodePoll,
        metadata: T::Metadata,
    },
    Poisoned,
}

impl<T, P, Q> EncodePoll<Q> for OwnEncoder<T,P,Q>
where Q: Ptr + Encode<Q>,
      P: Ptr + Encode<Q>,
      T: ?Sized + Save<Q> + Load<P>,
{
    const TARGET_BLOB_LAYOUT: BlobLayout = Q::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);
    type Target = Own<T,Q>;

    fn poll<D: Dumper<Q>>(&mut self, dumper: D) -> Result<D, D::Pending> {
        match self {
            Self::Own(_) => {
                let owned = if let Self::Own(owned) = mem::replace(self, Self::Poisoned) { owned } else { unreachable!() };
                let metadata = owned.metadata;

                *self = match P::encode_own(owned) {
                    Ok(ptr_encoder) => Self::Done { ptr_encoder, metadata },
                    Err(value_saver) => Self::SaveValue(value_saver),
                };

                self.poll(dumper)
            },
            Self::SaveValue(saver) => {
                let (dumper, ptr_encoder, metadata) = saver.poll(dumper)?;
                let ptr_encoder = D::coerce_ptr_encoder(ptr_encoder);
                *self = Self::Done { ptr_encoder, metadata };
                Ok(dumper)
            },
            Self::Done { .. } => Ok(dumper),
            Self::Poisoned => panic!("{} poisoned", type_name::<Self>()),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        if let Self::Done { ptr_encoder, metadata } = self {
            dst.write(ptr_encoder)?
               .write_primitive(metadata)?
               .finish()
        } else {
            panic!()
        }
    }
}

/*
impl<T: ?Sized + Pointee, Z: Zone> Own<T,Z> {
    pub unsafe fn from_raw_parts(ptr: Z::Ptr, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr, metadata
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }

    pub fn ptr(&self) -> &Z::Ptr {
        &self.ptr
    }
}


impl<T: ?Sized + Pointee, Z: Zone> Encode<Z> for Own<T,Z>
where T: Save<Z>
{
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(0);

    type Encode = OwnEncoder<T, Z>;

    fn encode(self) -> Self::Encode {
        OwnEncoder::Own(self)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> EncodePoll for OwnEncoder<T,Z>
where T: Save<Z>
{
    type Zone = Z;
    type Target = Own<T, Z>;

    fn poll<S>(&mut self, ptr_saver: &mut S) -> Poll<Result<(), S::Error>>
        where S: Saver<Zone = Z>
    {
        match self {
            OwnEncoder::Own(_) => {
                if let OwnEncoder::Own(own) = mem::replace(self, OwnEncoder::Poisoned) {
                    todo!()
                } else {
                    unreachable!()
                }
            },
            OwnEncoder::Save(saver) => {
                match saver.poll(ptr_saver)? {
                    Poll::Ready((persist_ptr, metadata)) => {
                        mem::replace(self, OwnEncoder::Done { persist_ptr, metadata });
                        Ok(()).into()
                    },
                    Poll::Pending => Poll::Pending,
                }
            },
            OwnEncoder::Done { .. } => Ok(()).into(),
            OwnEncoder::Poisoned => panic!("{} poisoned", type_name::<Self>()),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}
*/
