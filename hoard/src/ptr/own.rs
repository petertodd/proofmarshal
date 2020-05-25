use std::any::{self, Any};
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::ptr;

use thiserror::Error;

use owned::Take;

use super::*;

use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::primitive::*;


#[repr(C)]
pub struct Own<T: ?Sized + Pointee, P: Ptr, M: 'static = <T as Pointee>::Metadata> {
    marker: PhantomData<T>,
    inner: Fat<T, P, M>
}

impl<T: ?Sized + Pointee, P: Ptr, M> AsRef<Fat<T, P, M>> for Own<T, P, M> {
    fn as_ref(&self) -> &Fat<T, P, M> {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M> Drop for Own<T, P, M> {
    fn drop(&mut self) {
        unsafe {
            let metadata: &dyn Any = &self.metadata;
            let metadata = metadata.downcast_ref().unwrap();
            self.raw.dealloc::<T>(*metadata)
        }
    }
}



impl<T: ?Sized + Pointee, P: Ptr, M> Deref for Own<T, P, M> {
    type Target = Fat<T, P, M>;

    fn deref(&self) -> &Self::Target {
        &self.as_ref()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M> fmt::Debug for Own<T, P, M>
where P: fmt::Debug,
      M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(any::type_name::<Self>())
            .field(&self.inner)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M> Own<T, P, M> {
    pub unsafe fn new_unchecked(inner: Fat<T, P, M>) -> Self {
        Self { marker: PhantomData, inner, }
    }

    pub fn into_inner(self) -> Fat<T, P, M> {
        let this = ManuallyDrop::new(self);
        unsafe { ptr::read(&this.inner) }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateOwnBlobError<P: fmt::Debug, M: fmt::Debug> {
    Ptr(P),
    Metadata(M),
}

impl<T: ?Sized + Pointee, P: Ptr, M> ValidateBlob for Own<T, P, M>
where P: ValidateBlob,
      M: ValidateBlob,
{
    type Error = ValidateOwnBlobError<P::Error, M::Error>;

    const BLOB_LEN: usize = P::BLOB_LEN + M::BLOB_LEN;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<P>().map_err(ValidateOwnBlobError::Ptr)?;
        blob.field::<M>().map_err(ValidateOwnBlobError::Metadata)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Y, T: ?Sized + Pointee, P: Ptr> Decode<Y> for Own<T, P>
where T::Metadata: Decode<Y>,
      P: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        let r = unsafe {
            Self {
                marker: PhantomData,
                inner: Fat {
                    _marker: PhantomData,
                    raw: blob.field_unchecked(),
                    metadata: blob.field_unchecked(),
                },
            }
        };
        blob.finish();
        r
    }
}

unsafe impl<T: ?Sized + Pointee, P: Ptr, M> Persist for Own<T, P, M>
where P: Persist,
      M: Persist,
{}

impl<R: Ptr, T: ?Sized + Pointee, P: Ptr, M> Encoded<R> for Own<T, P, M>
where R: Primitive,
      T: Saved<R>,
      M: Primitive,
{
    type Encoded = Own<T::Saved, R, M>;
}

#[derive(Debug)]
pub enum EncodeOwnState<'a, R, T: ?Sized, TState> {
    Ready,
    Poll {
        value: &'a T,
        value_state: TState,
    },
    SaveBlob {
        value: &'a T,
        value_state: TState,
    },
    Done(R),
}

impl<'a, Q: 'a, R: 'a + Ptr, T: 'a + ?Sized + Pointee, P: Ptr> Encode<'a, Q, R> for Own<T, P>
where R: Primitive,
      T: Save<'a, Q, R>,
      P: std::borrow::Borrow<Q>,
{
    type State = EncodeOwnState<'a, R, T, T::State>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnState::Ready
    }

    fn encode_poll<D>(&'a self, state: &mut Self::State, mut dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        loop {
            *state = match state {
                EncodeOwnState::Ready => {
                    match unsafe { dst.try_save_ptr::<T>(self.raw.borrow(), self.metadata) } {
                        Ok(r_ptr) => EncodeOwnState::Done(r_ptr),
                        Err(value) => EncodeOwnState::Poll {
                            value_state: value.init_save_state(),
                            value,
                        },
                    }
                },
                EncodeOwnState::Poll { value, value_state } => {
                    dst = value.save_poll(value_state, dst)?;

                    EncodeOwnState::SaveBlob {
                        value,
                        value_state: mem::replace(value_state, value.init_save_state()),
                    }
                },
                EncodeOwnState::SaveBlob { value, value_state } => {
                    let (d, r_ptr) = dst.save_ptr::<T>(value, value_state)?;
                    dst = d;
                    EncodeOwnState::Done(r_ptr)
                },
                EncodeOwnState::Done(_) => break Ok(dst),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error>
        where R: ValidateBlob
    {
        if let EncodeOwnState::Done(r_ptr) = state {
            dst.write_primitive(r_ptr)?
               .write_primitive(&self.metadata)?
               .done()
        } else {
            panic!()
        }
    }
}
