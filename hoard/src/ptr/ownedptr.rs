use std::any::{self, Any};
use std::fmt;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr;

use thiserror::Error;

use owned::Take;

use super::*;

use crate::load::*;

pub struct OwnedPtr<T: ?Sized + Pointee, P: Ptr, M: 'static = <T as Pointee>::Metadata> {
    marker: PhantomData<T>,
    inner: FatPtr<T, P, M>,
}

impl<T: ?Sized + Pointee, P: Ptr, M: 'static> Drop for OwnedPtr<T, P, M> {
    fn drop(&mut self) {
        unsafe {
            let metadata: &dyn Any = &self.inner.metadata;
            let metadata = metadata.downcast_ref().unwrap();
            self.inner.raw.dealloc::<T>(*metadata)
        }
    }
}


impl<T: ?Sized + Pointee, P: Ptr, M: 'static> Deref for OwnedPtr<T, P, M> {
    type Target = FatPtr<T, P, M>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr, M: 'static> fmt::Debug for OwnedPtr<T, P, M>
where P: fmt::Debug,
      M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(any::type_name::<Self>())
            .field(&self.inner)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr> OwnedPtr<T, P> {
    pub unsafe fn new_unchecked(inner: FatPtr<T, P>) -> Self {
        Self {
            marker: PhantomData,
            inner,
        }
    }

    pub fn into_fatptr(self) -> FatPtr<T, P> {
        let this = ManuallyDrop::new(self);
        unsafe {
            ptr::read(&this.inner)
        }
    }

    pub unsafe fn get_fatptr_mut(&mut self) -> &mut FatPtr<T, P> {
        &mut self.inner
    }
}

impl<T, P: Ptr, M: 'static> Clone for OwnedPtr<T, P, M>
where T: Clone, P: Clone,
{
    fn clone(&self) -> Self {
        /*
        unsafe {
            OwnedPtr::new_unchecked(FatPtr::new(
                    self.raw.clone_unchecked::<T>(),
                    ()
            ))
        }
        */ todo!()
    }
}

pub type ValidateBlobOwnedPtrError<P, M> = super::fatptr::ValidateBlobFatPtrError<P, M>;

impl<T: ?Sized + Pointee, P: Ptr, M: 'static> ValidateBlob for OwnedPtr<T, P, M>
where P: ValidateBlob,
      M: ValidateBlob,
{
    type Error = ValidateBlobOwnedPtrError<P::Error, M::Error>;

    const BLOB_LEN: usize = <FatPtr<T, P, M> as ValidateBlob>::BLOB_LEN;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_fields();
        blob.validate::<FatPtr<T, P, M>>()?;
        unsafe { Ok(blob.assume_valid()) }
    }
}

unsafe impl<T: ?Sized + Pointee, P: Ptr, M: 'static> Persist for OwnedPtr<T, P, M>
where P: Persist, M: Persist,
{}

impl<Z, T: ?Sized + Pointee, P: Ptr, M: 'static> Load<Z> for OwnedPtr<T, P, M>
where P: Decode<Z>,
      M: Decode<Z>,
{
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, zone: &Z) -> Self::Owned {
        let mut blob = blob.into_loader(zone);

        unsafe {
            OwnedPtr {
                marker: PhantomData,
                inner: blob.decode_unchecked(),
            }
        }
    }
}

/*
impl<T: ?Sized + Pointee, P: Ptr> Default for OwnedPtr<T, P>
where T: Default, P: Default,
{
    fn default() -> Self {
        P::alloc(T::default())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
*/
