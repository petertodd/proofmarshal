/*
//! Pile pointers.
//!
//! # Lifetimes
//!
//! Both piles and offsets have *two* lifetime parameters: `'s` and `'p`. For reasons we'll see
//! later, these two parameters are independent of each other, with no outlives relationship.
//!
//! Let's look at how they work:
//!
//! ## `'p`
//!
//! An `Offset` needs the assistance of a `Pile` to actually get the data it points too. But since
//! it's just a 64-bit integer, there's no way to check at run-time if we're using the correct
//! pile. Instead, we use the `'p` lifetime to ensure this *at compile time*.
//!
//! This is achieved by:
//!
//! 1) Making `Pile<'p, 'v>` and `Offset<'p, 'v>` *invariant* over `'p`.
//! 2) Ensuring via the `Unique` API that exactly one pile can exist for a given `'p` lifetime.
//!
//! This means that the following won't even compile because `Pile<'s, 'static>` and `Pile<'p, 'v>`
//! are incompatible types:
//!
//! ```compile_fail
//! # use hoard::pile::{Pile, Offset};
//! fn foo<'p, 'v>(pile: Pile<'p, 'v>, offset: Offset<'p, 'v>) {}
//!
//! fn bar<'p, 'v>(pile: Pile<'s, 'static>, offset: Offset<'p, 'v>) {
//!     foo(pile, offset)
//! }
//! ```
//! similarly `Offset<'s, 'static>` and `Offset<'p, 'v>` are incompatible:
//!
//! ```compile_fail
//! # use hoard::pile::{Pile, Offset};
//! # fn foo<'p, 'v>(pile: Pile<'p, 'v>, offset: Offset<'p, 'v>) {}
//! fn bar<'p, 'v>(pile: Pile<'p, 'v>, offset: Offset<'s, 'static>) {
//!     foo(pile, offset)
//! }
//! ```
//!
//! ## `'s`
//!
//! We haven't talked about the actual underlying byte slice. That's because `'p` is solely a
//! compile-time check, with no other role. It's the `'s` parameter that ensures that the byte
//! slice lives longer than the data we we load from it.
//!
//! Recall that `Get` works along the the lines of the following simplified API:
//!
//! ```
//! # trait Load<P> {}
//! trait Get<P> {
//!     fn get<'a, T: Load<P>>(&self, ptr: &'a P) -> &'a T;
//! }
//! ```
//!
//! Note the lifetimes! It's the pointer's job to "own" that data, so `Offset<'p, 'v>` owns a
//! phantom `&'s [u8]`.
*/

use std::alloc::Layout;
use std::cmp;
use std::convert::TryInto;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::num::NonZeroU64;
use std::ptr::NonNull;

use thiserror::Error;
use leint::Le;

use crate::pointee::Pointee;
use crate::ptr::Ptr;
use crate::load::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'pile, 'version> {
    marker: PhantomData<(
        fn(&'pile [u8]) -> &'pile [u8],
        &'version (),
    )>,
    pub(super) raw: Le<NonZeroU64>,
}

impl<'p, 'v> Ptr for Offset<'p, 'v> {
    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, _metadata: T::Metadata) {
        // nothing to do here
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        *self
    }

    unsafe fn fmt_debug_valid_ptr<T: ?Sized + Pointee>(&self, metadata: T::Metadata, f: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }
}

#[derive(Debug, Error)]
#[error("invalid Offset: {0}")]
pub struct LoadOffsetError(u64);

impl<'p, 'v> Load for Offset<'p, 'v> {
    type Error = LoadOffsetError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        /*
        blob.validate_bytes(|blob| {
            let raw = u64::from_le_bytes(blob[..].try_into().unwrap());

            if raw & 1 == 0 {
                Err(LoadOffsetError(raw))
            } else {
                let idx = raw >> 1;
                if idx <= Self::MAX as u64 {
                    unsafe { Ok(blob.assume_valid()) }
                } else {
                    Err(LoadOffsetError(raw))
                }
            }
        })
        */ todo!()
    }
}

impl fmt::Debug for Offset<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.raw.get().get() & 1 == 1);
        <usize as fmt::Debug>::fmt(&self.get(), f)
    }
}

impl<'p,'v> Offset<'p,'v> {
    pub const MAX: usize = (1 << 62) - 1;

    #[inline(always)]
    pub fn new(offset: usize) -> Option<Self> {
        let offset = offset as u64;
        offset.checked_shl(1).map(|offset|
            Self {
                marker: PhantomData,
                raw: NonZeroU64::new(offset | 1).unwrap().into(),
            }
        )
    }

    /// Converts the `Offset` into an offset with a different lifetime.
    #[inline(always)]
    pub fn cast<'p2,'v2>(&self) -> Offset<'p2, 'v2> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    #[inline(always)]
    pub fn get(&self) -> usize {
        (self.raw.get().get() >> 1) as usize
    }

    pub fn dangling() -> Self {
        Self::new(Self::MAX).unwrap()
    }
}

impl From<Offset<'_, '_>> for usize {
    #[inline(always)]
    fn from(offset: Offset<'_,'_>) -> usize {
        offset.get()
    }
}

impl fmt::Pointer for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::TryPile;

    use crate::coerce::Coerce;
    use crate::zone::FatPtr;

    #[test]
    fn coerce_lifetimes() {
        fn do_coerce<'p,'v, T, U>(_: &'p Vec<u8>, ptr: FatPtr<T, TryPile<'p,'static>>) -> FatPtr<U, TryPile<'v, 'p>> {
            ptr.coerce()
        }

        let ptr = FatPtr::<Box<u8>, TryPile> {
            raw: Offset::<'_, 'static>::new(42).unwrap(),
            metadata: ()
        };

        let anchor = vec![];

        let ptr2: FatPtr<&Vec<u8>, TryPile<'static,'_>> = do_coerce(&anchor, ptr);

        assert_eq!(ptr2.raw.get(), 42);
    }
}
*/
