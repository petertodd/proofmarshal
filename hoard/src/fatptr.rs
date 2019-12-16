use super::*;

use core::fmt;
use core::cmp;
use core::hash;

use crate::marshal::{Persist, Primitive, blob::*};

use crate::coerce::{
    CastRef,
    TryCast, TryCastRef, TryCastMut
};

/// A zone pointer with metadata. *Not* necessarily valid.
#[repr(C)]
pub struct FatPtr<T: ?Sized + Pointee, P> {
    pub raw: P,
    pub metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, P> NonZero for FatPtr<T,P>
where P: NonZero {}

/// Implemented for all `P: Persist` because metadata is always `Persist`.
unsafe impl<T: ?Sized + Pointee, P> Persist for FatPtr<T,P>
where P: Persist,
{}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidateFatPtrError<T, P> {
    Ptr(P),
    Metadata(T),
}

impl<T: ?Sized + Pointee, P> Primitive for FatPtr<T,P>
where P: Primitive
{
    type Error = ValidateFatPtrError<<T::Metadata as Primitive>::Error, P::Error>;
    const BLOB_LAYOUT: BlobLayout = P::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.raw)?
           .write_primitive(&self.metadata)?
           .finish()
    }

    fn validate_blob<'a, Q: Ptr>(blob: Blob<'a, Self, Q>) -> Result<FullyValidBlob<'a, Self, Q>, Self::Error> {
        let mut fields = blob.validate_primitive_struct();
        fields.field::<P>().map_err(ValidateFatPtrError::Ptr)?;
        fields.field::<T::Metadata>().map_err(ValidateFatPtrError::Metadata)?;

        unsafe { Ok(fields.assume_done()) }
    }
}

impl<T, P> From<P> for FatPtr<T,P> {
    fn from(raw: P) -> Self {
        Self { raw, metadata: () }
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastRef<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&FatPtr<T,Q>, Self::Error> {
        match self.raw.try_cast_ref() {
            Err(e) => Err(e),
            Ok(_) => Ok(unsafe { &*(self as *const _ as *const _) })
        }
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastMut<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCastMut<Q>
{
    fn try_cast_mut(&mut self) -> Result<&mut FatPtr<T,Q>, Self::Error> {
        match self.raw.try_cast_mut() {
            Err(e) => Err(e),
            Ok(_) => Ok(unsafe { &mut *(self as *mut _ as *mut _) })
        }
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCast<FatPtr<T,Q>> for FatPtr<T,P>
where P: TryCast<Q>
{}

impl<T: ?Sized + Pointee, P, Q> AsRef<FatPtr<T,Q>> for FatPtr<T,P>
where P: CastRef<Q>
{
    fn as_ref(&self) -> &FatPtr<T,Q> {
        unsafe {
            &*(self as *const _ as *const _)
        }
    }
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
