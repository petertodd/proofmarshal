use std::fmt;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops;

use thiserror::Error;

use crate::load::*;

use super::*;

pub struct Bag<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<Box<T>>,
    inner: Fat<T, P>,
}

impl<T: ?Sized + Pointee, P: Ptr> Bag<T,P> {
    pub unsafe fn new_unchecked(ptr: Fat<T,P>) -> Self {
        Self {
            marker: PhantomData,
            inner: ptr,
        }
    }

    pub fn into_inner(self) -> Fat<T,P> {
        let this = ManuallyDrop::new(self);

        unsafe { std::ptr::read(&this.inner) }
    }

    pub unsafe fn raw_mut(&mut self) -> &mut P {
        &mut self.inner.raw
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for Bag<T, P> {
    fn drop(&mut self) {
        unsafe {
            self.inner.raw.dealloc::<T>(self.inner.metadata)
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> ops::Deref for Bag<T, P> {
    type Target = Fat<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}


impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for Bag<T, P>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            P::fmt_debug_valid_ptr::<T>(&self.raw, self.metadata, f)
        }
    }
}

#[derive(Error, Debug)]
pub enum LoadBagError<P: std::fmt::Debug, M: std::fmt::Debug, L: std::fmt::Debug> {
    #[error("invalid pointer: {0:?}")]
    Pointer(P),

    #[error("invalid metadata: {0:?}")]
    Metadata(M),

    #[error("layout error: {0:?}")]
    Layout(L),
}

impl<T: ?Sized + Pointee, P: Ptr> Load for Bag<T, P>
where P: Load
{
    type Error = LoadBagError<P::Error, <T::Metadata as Load>::Error, T::LayoutError>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}
