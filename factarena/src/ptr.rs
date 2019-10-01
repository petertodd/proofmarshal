/// Generic pointers

use core::any::type_name;
use core::cmp;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

use crate::pointee::{Metadata, Pointee};

/// Typed wrapper around a raw, untyped, pointer.
///
/// Contains the raw pointer and any required type metadata.
///
/// This exists to reason about safety: the only way to create of a `Ptr<T,P>` is with `unsafe`,
/// thus any code creates one is asserting that the `P` value is valid for the given type `T`.
#[repr(C)]
pub struct Ptr<T: ?Sized + Pointee, P> {
    marker: PhantomData<*const T>,
    raw: P,
    metadata: T::Metadata,
}

impl<T: ?Sized + Pointee, P> Ptr<T,P> {
    /// Creates a new `Ptr<T,P>`.
    ///
    /// # Safety
    ///
    /// You are asserting that the pointer is appropriate for the declared type `T`.
    pub unsafe fn new(raw: P, metadata: T::Metadata) -> Self {
        Ptr {
            marker: PhantomData,
            raw, metadata,
        }
    }

    /// Deconstructs the `Ptr` into its raw parts.
    pub fn into_raw(self) -> (P, T::Metadata) {
        (self.raw, self.metadata)
    }

    /// Accesses the underlying raw pointer.
    pub fn raw(&self) -> &P {
        &self.raw
    }

    /// Gets the metadata.
    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<T: ?Sized + Pointee, P> fmt::Debug for Ptr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct(type_name::<Self>())
         .field("raw", &self.raw)
         .field("metadata", &self.metadata)
         .finish()
    }
}

// Standard trait impls for Ptr.
//
// Can't use derive because whether Ptr implements these traits should be independent of whether T
// does.

impl<T: ?Sized + Pointee, P> Default for Ptr<T,P>
where T::Metadata: Default,
      P: Default,
{
    #[inline]
    fn default() -> Self {
        unsafe {
            Self::new(P::default(), T::Metadata::default())
        }
    }
}

impl<T: ?Sized + Pointee, P> Clone for Ptr<T,P>
where P: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        unsafe {
            Self::new(self.raw.clone(), self.metadata)
        }
    }
}

/*
impl<T: ?Sized + Pointee, R: Dealloc> cmp::PartialEq for Ptr<T,R>
where R: cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.metadata.eq(&other.metadata) && self.raw.eq(&other.raw)
    }
}
impl<T: ?Sized + Pointee, R: Dealloc> cmp::Eq for Ptr<T,R>
where R: cmp::Eq {}


/// Generic missing pointer
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing;

impl Dealloc for Missing {
    unsafe fn dealloc<T: ?Sized + Pointee>(self, _: T::Metadata) {}
}*/
