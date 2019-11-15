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

pub enum EncodeOwnState<T: ?Sized + Save<Q>, Q: Encode<Q>> {
    Initial,
    Value(T::State),
    Done {
        ptr_state: Q::State,
        metadata: T::Metadata,
    },
}

impl<T, P, Q> Encode<Q> for Own<T,P>
where Q: Ptr + Encode<Q>,
      P: Ptr + Encode<Q>,
      T: ?Sized + Save<Q>,
{
    const BLOB_LAYOUT: BlobLayout = Q::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type State = EncodeOwnState<T, Q>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnState::Initial
    }

    fn encode_poll<D: Dumper<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending> {
        match state {
            EncodeOwnState::Initial => {
                *state = match P::encode_own(self) {
                    Ok(ptr_state) => EncodeOwnState::Done { ptr_state, metadata: self.metadata },
                    Err(value_state) => EncodeOwnState::Value(value_state),
                };

                self.encode_poll(state, dumper)
            },
            EncodeOwnState::Value(value_state) => {
                let metadata = self.metadata;
                let (dumper, ptr_state) = P::encode_own_value(self, value_state, dumper)?;

                *state = EncodeOwnState::Done { ptr_state, metadata };
                Ok(dumper)
            },
            EncodeOwnState::Done { .. } => Ok(dumper),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeOwnState::Done { ptr_state, metadata } = state {
            /*
            let ptr_writer = ValueWriter::new(dst, Q::BLOB_LAYOUT.size());
            let dst = Q::encode_ptr(ptr_state, ptr_writer)?;

            dst.write_primitive(metadata)?
               .finish()
            */
            todo!()
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
