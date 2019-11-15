use super::*;

use core::fmt;
use core::cmp;
use core::hash;

use crate::marshal::{
    Persist, Primitive,
    blob::{
        BlobLayout,
        Blob, FullyValidBlob,
        ValidatePrimitiveFields,
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


impl<T: ?Sized + Pointee, P> Primitive for FatPtr<T,P>
where P: Primitive,
{
    type Error = FatPtrError<T,P>;

    const BLOB_LAYOUT: BlobLayout = P::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.raw)?
           .write_primitive(&self.metadata)?
           .finish()
    }

    fn validate_blob<'p, Q>(blob: Blob<'p, Self, Q>) -> Result<FullyValidBlob<'p, Self, Q>, Self::Error> {
        let mut fields = blob.validate_primitive_struct();

        fields.field::<P>().map_err(FatPtrError::Ptr)?;
        fields.field::<T::Metadata>().map_err(FatPtrError::Metadata)?;

        unsafe { Ok(fields.assume_fully_valid()) }
    }

    fn decode_blob<'p, Q>(blob: FullyValidBlob<'p, Self, Q>) -> Self {
        todo!()
    }
}

pub enum FatPtrError<T: ?Sized + Pointee, P: Primitive> {
    Ptr(P::Error),
    Metadata(<T::Metadata as Primitive>::Error),
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
