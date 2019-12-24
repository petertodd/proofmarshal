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

use core::cmp;
use core::convert::TryInto;
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use std::alloc::Layout;

use leint::Le;
use nonzero::NonZero;

use super::Pile;

use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::marshal::*;
use crate::zone::Zone;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'pile, 'version> {
    marker: PhantomData<(
        for<'a> fn(&'a Pile<'pile, 'version>) -> &'a (),
        &'version (),
    )>,
    raw: Le<NonZeroU64>,
}

impl fmt::Debug for Offset<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.raw.get().get() & 1 == 1);
        <usize as fmt::Debug>::fmt(&self.get(), f)
    }
}

unsafe impl NonZero for Offset<'_,'_> {}
unsafe impl NonZero for OffsetMut<'_,'_> {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p,'v>(Offset<'p,'v>);


#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'p,'v> {
    Offset(Offset<'p,'v>),
    Ptr(NonNull<u16>),
}

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

impl<'p, 'v> From<Offset<'p,'v>> for OffsetMut<'p,'v> {
    #[inline]
    fn from(offset: Offset<'p,'v>) -> Self {
        Self(offset)
    }
}

impl fmt::Pointer for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

impl Persist for Offset<'_,'_> {
    type Persist = Offset<'static, 'static>;
}

impl Persist for OffsetMut<'_,'_> {
    type Persist = Offset<'static, 'static>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateOffsetError(u64);

impl ValidationError for ValidateOffsetError {
}

impl ValidateBlob for Offset<'_,'_> {
    type Error = ValidateOffsetError;

    #[inline]
    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
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

unsafe impl<'a, Z> ValidateChildren<'a, Z> for Offset<'_, '_> {
    type State = ();
    fn validate_children(_: &Offset<'static, 'static>) -> () {}

    fn poll<V: PtrValidator<Z>>(this: &'a Offset<'static, 'static>, _: &mut (), _: &V) -> Result<&'a Self, V::Error> {
        Ok(unsafe { mem::transmute::<&Offset, &Offset>(this) })
    }
}

impl<Z> Decode<Z> for Offset<'_, '_> {}

impl<Z> Encoded<Z> for Offset<'_, '_> {
    type Encoded = Self;
}

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


impl<'s,'m> OffsetMut<'s,'m> {
    #[inline]
    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        debug_assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

    #[inline]
    pub fn kind(&self) -> Kind<'s,'m> {
        if self.0.raw.get().get() & 1 == 1 {
            Kind::Offset(self.0)
        } else {
            Kind::Ptr(unsafe {
                let raw = self.0.raw.get().get();
                NonNull::new_unchecked(raw as usize as *mut u16)
            })
        }
    }

    #[inline(always)]
    pub fn get_offset(&self) -> Option<Offset<'s,'m>> {
        match self.kind() {
            Kind::Offset(offset) => Some(offset),
            Kind::Ptr(_) => None,
        }
    }

    #[inline(always)]
    pub fn get_ptr(&self) -> Option<NonNull<u16>> {
        match self.kind() {
            Kind::Ptr(ptr) => Some(ptr),
            Kind::Offset(_) => None,
        }
    }
}

impl fmt::Debug for OffsetMut<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.kind(), f)
    }
}

impl fmt::Pointer for OffsetMut<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
