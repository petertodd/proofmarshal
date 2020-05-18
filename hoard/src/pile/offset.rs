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
use crate::ptr::*;
use crate::zone::*;
use crate::refs::*;
use crate::blob::*;
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
    type Persist = Offset<'static, 'static>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, _: T::Metadata) {
        // nothing to do here
    }

    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        *self
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        todo!()
    }
}

#[derive(Debug, Error)]
#[error("invalid offset")]
#[non_exhaustive]
pub struct ValidateBlobOffsetError;

impl ValidateBlob for Offset<'_, '_> {
    type Error = ValidateBlobOffsetError;

    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let raw = u64::from_le_bytes(blob[..].try_into().unwrap());

        if raw & 1 == 0 {
            Err(ValidateBlobOffsetError)
        } else {
            let idx = raw >> 1;
            if idx <= Self::MAX as u64 {
                unsafe { Ok(blob.assume_valid()) }
            } else {
                Err(ValidateBlobOffsetError)
            }
        }
    }
}

impl<Z> Load<Z> for Offset<'_, '_> {
    fn decode_blob_owned<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Self {
        blob.to_ref().clone()
    }

    fn load_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Ref<'a, Self> {
        blob.to_ref().into()
    }
}

unsafe impl Persist for Offset<'_, '_> {
}

/*
impl<'p, 'v, R> Saved<R> for Offset<'p, 'v> {
    type Saved = Offset<'p, 'v>;
}

impl<'p, 'v, Q, R> Save<'_, Q, R> for Offset<'p, 'v> {
    type State = ();

    fn init_save_state(&self) -> Self::State {}

    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut (), dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        dst.write_primitive(&self.raw)?
           .done()
    }
}

impl<'p, 'v> Primitive for Offset<'p, 'v> {}
*/

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(Offset::dangling().get(), Offset::MAX);
    }
}
