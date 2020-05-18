use std::any::type_name;
use std::fmt;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;

use thiserror::Error;

use owned::{Take, IntoOwned};

use crate::pointee::Pointee;
use crate::refs::*;
use crate::ptr::*;
use crate::load::*;

#[derive(Debug)]
pub struct Bag<T: ?Sized + Pointee, Z, P: Ptr, M: 'static = <T as Pointee>::Metadata> {
    ptr: OwnedPtr<T, P, M>,
    zone: Z,
}

impl<T: ?Sized + Pointee, Z, P: Ptr> Bag<T, Z, P> {
    pub fn from_owned_ptr(ptr: OwnedPtr<T, P>, zone: Z) -> Self {
        Self {
            ptr,
            zone,
        }
    }
}

/*
impl<T: ?Sized + Pointee, Z, P: Ptr> Bag<T, Z, P>
where T: Load<Z, P>
{
    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where Z: Get<P>
    {
        unsafe {
            Ref::Ref(self.zone.get_unchecked(&self.ptr.raw, self.ptr.metadata))
        }
    }
}
*/

/*
impl<T: ?Sized + Pointee, Z: Zone> Bag<T, Z>
where T: Load<Z>
{
    pub fn get<'a>(&'a self) -> Ref<'a, T>
        where Z: Get
    {
        unsafe {
            self.zone.get_unchecked::<T>(&self.ptr, self.metadata)
        }
    }

    pub fn get_mut<'a>(&'a mut self) -> &'a mut T
        where Z: GetMut
    {
        unsafe {
            self.zone.get_mut_unchecked::<T>(&mut self.ptr, self.metadata)
        }
    }

    pub fn take<'a>(self) -> T::Owned
        where Z: Get
    {
        let (ptr, metadata, zone) = self.into_raw_parts();
        unsafe {
            zone.take_unchecked::<T>(ptr, metadata)
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Bag<T, Z> {
    pub fn into_raw_parts(self) -> (Z::Ptr, T::Metadata, Z) {
        let this = ManuallyDrop::new(self);
        unsafe {
            (ptr::read(&this.ptr),
             this.metadata,
             ptr::read(&this.zone))
        }
    }

    pub unsafe fn from_raw_parts(ptr: Z::Ptr, metadata: T::Metadata, zone: Z) -> Self {
        Self {
            marker: PhantomData,
            ptr, metadata, zone,
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Clone for Bag<T, Z>
where T: Clone, Z: Clone,
{
    fn clone(&self) -> Self {
        unsafe {
            Self::from_raw_parts(
                Z::clone_ptr_unchecked::<T>(&self.ptr),
                self.metadata,
                self.zone.clone()
            )
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Default for Bag<T, Z>
where T: Default, Z: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}
*/

// serialization

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateBlobBagError<P: fmt::Debug, Z: fmt::Debug> {
    Ptr(P),
    Zone(Z),
}

impl<T: ?Sized + Pointee, Z, P: Ptr, M: 'static> ValidateBlob for Bag<T, Z, P, M>
where Z: ValidateBlob,
      P: ValidateBlob,
      M: ValidateBlob,
{
    type Error = ValidateBlobBagError<
                    <OwnedPtr<T, P, M> as ValidateBlob>::Error,
                    Z::Error,
                 >;

    const BLOB_LEN: usize = <OwnedPtr<T, P, M> as ValidateBlob>::BLOB_LEN + Z::BLOB_LEN;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_fields();
        blob.validate::<OwnedPtr<T, P, M>>().map_err(ValidateBlobBagError::Ptr)?;
        blob.validate::<Z>().map_err(ValidateBlobBagError::Zone)?;
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<Y, T: ?Sized + Pointee, Z, P: Ptr, M: 'static> Load<Y> for Bag<T, Z, P, M>
where Z: Decode<Y>,
      P: Decode<Y>,
      M: Decode<Y>,
{
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Y) -> Self {
        let mut blob = blob.into_loader(zone);
        unsafe {
            let ptr = blob.decode_unchecked();
            let zone = blob.decode_unchecked();
            blob.assert_done();
            Self { ptr, zone }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::heap::Heap;

    #[test]
    fn test() {
    }
}
