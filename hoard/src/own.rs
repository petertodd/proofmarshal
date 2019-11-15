use super::*;

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;

use crate::marshal::*;
use crate::marshal::blob::*;

/// An owned pointer.
#[repr(C)]
pub struct Own<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<T>,
    inner: ManuallyDrop<FatPtr<T,P>>,
}

impl<T: ?Sized + Pointee, P: Ptr> ops::Deref for Own<T,P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &FatPtr<T,P> {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Own<T,P> {
    pub unsafe fn new_unchecked(ptr: FatPtr<T,P>) -> Self {
        Self {
            marker: PhantomData,
            inner: ManuallyDrop::new(ptr),
        }
    }

    pub fn into_inner(self) -> FatPtr<T,P> {
        let mut this = ManuallyDrop::new(self);

        unsafe { (&mut *this.inner as *mut FatPtr<T,P>).read() }
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
        fmt::Pointer::fmt(&*self.inner, f)
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
where Q: Encode<Q>,
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

pub enum DecodeOwnError<T: ?Sized + Load<Q>, Q: Decode<Q>> {
    Ptr(Q::Error),
    Metadata(<T::Metadata as Primitive>::Error),
}

pub enum OwnValidator<T: ?Sized + Load<Q>, Q: Decode<Q>> {
    FatPtr(FatPtr<T, Q>),
    Value(T::ValidateChildren),
}

impl<T: ?Sized + Pointee, P: Ptr, Q> Decode<Q> for Own<T,P>
where T: Load<Q>,
      P: Decode<Q>,
      Q: Decode<Q>,
{
    type Error = DecodeOwnError<T,Q>;

    type ValidateChildren = OwnValidator<T,Q>;
    fn validate_blob<'a>(blob: Blob<'a, Self, Q>) -> Result<BlobValidator<'a, Self, Q>, Self::Error> {
        todo!()
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Q>, loader: &impl LoadPtr<Q>) -> Self {
        todo!()
    }
}

impl<T: ?Sized + Load<Q>, Q: Decode<Q>> ValidateChildren<Q> for OwnValidator<T,Q> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Q>
    {
        match self {
            Self::FatPtr(ptr) => {
                todo!()
            },
            Self::Value(value) => value.validate_children(validator),
        }
    }
}
