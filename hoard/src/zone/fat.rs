use std::any;
use std::fmt;
use std::cmp;
use std::marker::PhantomData;

use thiserror::Error;

use super::*;

use crate::pointee::Pointee;
use crate::blob::{self, Blob, ValidBlob, Persist};
use crate::load::*;
//use crate::save::*;

#[repr(C)]
pub struct Fat<T: ?Sized, P, M = <T as Pointee>::Metadata> {
    pub _marker: PhantomData<*const T>,
    pub raw: P,
    pub metadata: M,
}

impl<T: ?Sized + Pointee, P> Fat<T, P> {
    pub fn new(raw: P, metadata: T::Metadata) -> Self {
        Self {
            _marker: PhantomData,
            raw, metadata,
        }
    }
}

impl<T: ?Sized, P, M> fmt::Debug for Fat<T, P, M>
where P: fmt::Debug,
      M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(any::type_name::<Self>())
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M: Clone> Clone for Fat<T, P, M> {
    fn clone(&self) -> Self {
        /*
        Self {
            _marker: PhantomData,
            raw: self.raw.duplicate(),
            metadata: self.metadata.clone(),
        }
        */ todo!()
    }
}

impl<T: ?Sized + Pointee, P1, P2> PartialEq<Fat<T, P2>> for Fat<T, P1>
where P1: PartialEq<P2>
{
    fn eq(&self, other: &Fat<T, P2>) -> bool {
        self.raw == other.raw
            && self.metadata == other.metadata
    }
}

impl<T: ?Sized + Pointee, P> Eq for Fat<T, P>
where P: Eq
{}

#[derive(Debug, Error)]
#[error("fixme")]
pub enum ValidateFatBlobError<P: fmt::Debug, M: fmt::Debug> {
    Ptr(P),
    Metadata(M),
}

/*
impl<T: ?Sized, P, M> blob::ValidateBlob for Fat<T, P, M>
where P: blob::ValidateBlob,
      M: blob::ValidateBlob,
{
    type Error = ValidateFatBlobError<P::Error, M::Error>;

    const BLOB_LAYOUT: blob::BlobLayout = P::BLOB_LAYOUT.extend(M::BLOB_LAYOUT);

    fn validate_blob<'a>(mut blob: blob::BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<P>().map_err(ValidateFatBlobError::Ptr)?;
        blob.field::<M>().map_err(ValidateFatBlobError::Metadata)?;
        unsafe { Ok(blob.finish()) }
    }
}

unsafe impl <T: ?Sized, P, M> Persist for Fat<T, P, M>
where P: Persist,
      M: Persist,
{}

impl<Z: Zone, T: ?Sized, P, M> Decode<Z> for Fat<T, P, M>
where P: Decode<Z>,
      M: Decode<Z>,
{
    fn decode_blob(mut blob: ValidBlob<Self>, zone: &Z) -> Self {
        /*
        unsafe {
            Fat {
                _marker: PhantomData,
                raw: blob.field_unchecked(),
                metadata: blob.field_unchecked(),
            }
        }
        */ todo!()
    }
}
*/

/*
impl<R, T: ?Sized, P, M> Encoded<R> for Fat<T, P, M>
where T: Saved<R>,
      M: ValidateBlob,
      R: ValidateBlob,
{
    type Encoded = Fat<T::Saved, R, M>;
}

impl<Q, R, T: ?Sized, P, M> Encode<'_, Q, R> for Fat<T, P, M>
where T: Saved<R>,
      R: ValidateBlob,
      M: ValidateBlob,
{
    type State = ();

    fn init_encode_state(&self) -> () {}

    fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}

/*
impl<T: ?Sized, P, M> Primitive for Fat<T, P, M>
where P: Primitive, M: Primitive
{}
*/
*/
