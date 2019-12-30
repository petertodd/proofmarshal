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
use nonzero::NonZero;

use crate::coerce::TryCoerce;
use crate::marshal::PtrValidator;
use crate::marshal::blob;
use crate::marshal::decode::*;

use super::Pile;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'pile, 'version> {
    marker: PhantomData<(
        fn(&Pile<'pile, 'version>) -> &'pile (),
        &'version (),
    )>,
    pub(super) raw: Le<NonZeroU64>,
}

impl fmt::Debug for Offset<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.raw.get().get() & 1 == 1);
        <usize as fmt::Debug>::fmt(&self.get(), f)
    }
}

unsafe impl NonZero for Offset<'_,'_> {}

impl<'p,'v> Offset<'p,'v> {
    pub const MAX: usize = (1 << 62) - 1;

    #[inline]
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
    #[inline]
    pub fn cast<'p2,'v2>(&self) -> Offset<'p2, 'v2> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    #[inline]
    pub fn get(&self) -> usize {
        (self.raw.get().get() >> 1) as usize
    }
}

impl From<Offset<'_, '_>> for usize {
    fn from(offset: Offset<'_,'_>) -> usize {
        offset.get()
    }
}

impl fmt::Pointer for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

#[derive(Error,Debug, PartialEq, Eq)]
#[error("invalid Offset: {0}")]
pub struct ValidateOffsetError(u64);

impl blob::Validate for Offset<'static, 'static> {
    type Error = ValidateOffsetError;

    fn validate<'a, V: blob::Validator>(blob: blob::Cursor<'a, Self, V>)
        -> Result<blob::ValidBlob<'a, Self>, blob::Error<Self::Error, V::Error>>
    {
        blob.validate_bytes(|blob| {
            let raw = u64::from_le_bytes(blob[..].try_into().unwrap());

            if raw & 1 == 0 {
                Err(ValidateOffsetError(raw))
            } else {
                let idx = raw >> 1;
                if idx <= Self::MAX as u64 {
                    unsafe { Ok(blob.assume_valid()) }
                } else {
                    Err(ValidateOffsetError(raw))
                }
            }
        })
    }
}

unsafe impl Persist for Offset<'_, '_> {
    type Persist = Offset<'static, 'static>;
    type Error = ValidateOffsetError;
}

unsafe impl<'a, Z> ValidateChildren<'a, Z> for Offset<'_, '_> {
    type State = ();
    fn validate_children(_: &Offset<'static, 'static>) -> () {}

    fn poll<V: PtrValidator<Z>>(this: &Self::Persist, _: &mut (), _: &V) -> Result<(), V::Error> {
        Ok(())
    }
}
impl<Z> Decode<Z> for Offset<'_,'_> {}

/*
impl<Z: Zone> Encode<'_, Z> for Offset<'_, '_> {
    type State = ();
    fn save_children(&self) -> () {}

    fn poll<D: Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_primitive(&self.raw)?
           .finish()
    }
}

impl Primitive for Offset<'static, 'static> {}
*/

unsafe impl<'p, 'v> TryCoerce<Offset<'p,'v>> for Offset<'_, '_> {
    type Error = !;
}

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
