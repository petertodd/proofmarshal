use super::*;

use core::fmt;
use core::cmp;
use core::hash;

use crate::marshal::{
    Encode, Decode, SavePtr, LoadPtr,
    Persist, Primitive,
    blob::{
        BlobLayout,
        Blob, BlobValidator, FullyValidBlob,
        ValidateFields,
        WriteBlob},
};

#[repr(C)]
pub struct FatPtr<T: ?Sized + Pointee, P> {
    pub raw: P,
    pub metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, P> Persist for FatPtr<T,P>
where P: Persist,
      T::Metadata: Persist,
{}

impl<T, P> From<P> for FatPtr<T,P> {
    fn from(raw: P) -> Self {
        Self { raw, metadata: () }
    }
}


impl<T: ?Sized + Pointee, P, Q> Encode<Q> for FatPtr<T,P>
where P: Encode<Q>
{
    const BLOB_LAYOUT: BlobLayout = P::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type State = P::State;

    fn init_encode_state(&self) -> Self::State {
        self.raw.init_encode_state()
    }

    fn encode_poll<D: SavePtr<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending> {
        self.raw.encode_poll(state, dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        dst.write(&self.raw, state)?
           .write_primitive(&self.metadata)?
           .finish()
    }
}

impl<T: ?Sized + Pointee, P, Q> Decode<Q> for FatPtr<T,P>
where P: Decode<Q>
{
    type Error = FatPtrError<P::Error, <T::Metadata as Primitive>::Error>;

    type ValidateChildren = P::ValidateChildren;

    fn validate_blob<'p>(blob: Blob<'p, Self, Q>) -> Result<BlobValidator<'p, Self, Q>, Self::Error> {
        let mut fields = blob.validate_struct();
        let state = fields.field::<P>().map_err(FatPtrError::Ptr)?;
        let _: () = fields.field::<T::Metadata>().map_err(FatPtrError::Metadata)?;

        Ok(fields.done(state))
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Q>, loader: &impl LoadPtr<Q>) -> Self {
        let mut fields = blob.decode_struct(loader);
        Self {
            raw: fields.field(),
            metadata: fields.field(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FatPtrError<P,M> {
    Ptr(P),
    Metadata(M),
}

// standard impls

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for FatPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FatPtr")
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<FatPtr<T,Q>> for FatPtr<T,P>
where P: cmp::PartialEq<Q>,
{
    fn eq(&self, other: &FatPtr<T,Q>) -> bool {
        (self.raw == other.raw) && (self.metadata == other.metadata)
    }
}

impl<T: ?Sized + Pointee, P> cmp::Eq for FatPtr<T,P>
where P: cmp::Eq,
{}

impl<T: ?Sized + Pointee, P> Clone for FatPtr<T,P>
where P: Clone,
{
    fn clone(&self) -> Self {
        Self { raw: self.raw.clone(), metadata: self.metadata }
    }
}

impl<T: ?Sized + Pointee, P> Copy for FatPtr<T,P>
where P: Copy,
{}

impl<T: ?Sized + Pointee, P> hash::Hash for FatPtr<T,P>
where P: hash::Hash,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.metadata.hash(state);
    }
}


impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for FatPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FatPtr")
            .field(&self.raw)
            .field(&self.metadata)
            .finish()
    }
}
