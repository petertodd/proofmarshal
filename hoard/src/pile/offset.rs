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

use core::any::Any;
use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::num::NonZeroU64;
use core::ptr::{self, NonNull};
use core::ops;

use std::alloc::Layout;

use leint::Le;

use super::*;

use crate::marshal::{
    Encode, Save, Dumper,
    Persist,
    blob::*,
};

use crate::coerce::{TryCast, TryCastRef, TryCastMut};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'pile, 'version> {
    marker: PhantomData<(
        for<'a> fn(&'a Pile<'pile, 'version>) -> &'a (),
        &'version (),
    )>,
    raw: Le<NonZeroU64>,
}

unsafe impl Persist for Offset<'_,'_> {}

impl fmt::Debug for Offset<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.raw.get().get() & 1 == 1);
        f.debug_tuple("Offset")
            .field(&self.get())
            .finish()
    }
}

impl From<Offset<'_, '_>> for usize {
    fn from(offset: Offset<'_,'_>) -> usize {
        offset.get()
    }
}

#[derive(Debug)]
pub struct OffsetError {
    pub(super) offset: usize,
    pub(super) size: usize,
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

    #[inline]
    pub fn to_static(&self) -> Offset<'static, 'static> {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p,'v>(Offset<'p,'v>);
unsafe impl Persist for OffsetMut<'_,'_> {}

unsafe impl<'p,'v> TryCastRef<OffsetMut<'p,'v>> for Offset<'p,'v> {
    type Error = !;

    #[inline(always)]
    fn try_cast_ref(&self) -> Result<&OffsetMut<'p,'v>, Self::Error> {
        // Safe because OffsetMut is a #[repr(transparent)] Offset
        Ok(unsafe { mem::transmute(self) })
    }
}

unsafe impl<'p,'v> TryCast<OffsetMut<'p,'v>> for Offset<'p,'v> {
    #[inline(always)]
    fn try_cast(self) -> Result<OffsetMut<'p,'v>, Self::Error> {
        // Safe because OffsetMut is a #[repr(transparent)] Offset
        Ok(unsafe { mem::transmute(self) })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TryCastOffsetMutError(());

unsafe impl<'p,'v> TryCastRef<Offset<'p,'v>> for OffsetMut<'p,'v> {
    type Error = TryCastOffsetMutError;

    #[inline]
    fn try_cast_ref(&self) -> Result<&Offset<'p,'v>, Self::Error> {
        match self.kind() {
            Kind::Offset(_) => Ok(&self.0),
            Kind::Ptr(_) => Err(TryCastOffsetMutError(())),
        }
    }
}

unsafe impl<'p,'v> TryCast<Offset<'p,'v>> for OffsetMut<'p,'v> {
    #[inline]
    fn try_cast(self) -> Result<Offset<'p,'v>, Self::Error> {
        self.try_cast_ref().map(|r| *r)
    }
}

impl<'p, 'v> From<Offset<'p,'v>> for OffsetMut<'p,'v> {
    #[inline]
    fn from(offset: Offset<'p,'v>) -> Self {
        Self(offset)
    }
}

impl<'p,'v> Ptr for Offset<'p, 'v> {
    type Persist = Self;
    type Zone = Pile<'p, 'v>;
    type Allocator = crate::never::NeverAllocator<Self>;

    #[inline]
    fn allocator() -> Self::Allocator {
        panic!()
    }

    #[inline]
    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        unsafe {
            OwnedPtr::new_unchecked(
                ValidPtr::new_unchecked(
                    FatPtr {
                        raw: ptr.raw,
                        metadata: ptr.metadata,
                    }
                )
            )
        }
    }

    #[inline]
    fn dealloc_owned<T: ?Sized + Pointee>(own: OwnedPtr<T, Self>) {
        let _ = own.into_inner();
    }

    #[inline]
    fn drop_take_unsized<T: ?Sized + Pointee>(_: OwnedPtr<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
    }

    #[inline]
    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self> {
        Err(ptr.raw)
    }
}

impl fmt::Pointer for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeOffsetError {
    Ptr(u64),
    OutOfRange(u64),
}

impl Primitive for Offset<'_,'_> {
    type Error = DecodeOffsetError;
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    #[inline]
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().get().to_le_bytes())?
           .finish()
    }

    #[inline]
    fn validate_blob<'a, P: Ptr>(blob: Blob<'a, Self, P>) -> Result<FullyValidBlob<'a, Self, P>, Self::Error> {
        let raw = u64::from_le_bytes(blob[..].try_into().unwrap());

        if raw & 1 == 0 {
            Err(DecodeOffsetError::Ptr(raw))
        } else {
            let raw = raw >> 1;
            if raw <= Self::MAX as u64 {
                unsafe { Ok(blob.assume_fully_valid()) }
            } else {
                Err(DecodeOffsetError::OutOfRange(raw))
            }
        }
    }

    #[inline]
    fn decode_blob<'a, P: Ptr>(blob: FullyValidBlob<'a, Self, P>) -> Self {
        <Self as Primitive>::deref_blob(blob).clone()
    }

    #[inline]
    fn load_blob<'a, P: Ptr>(blob: FullyValidBlob<'a, Self, P>) -> Ref<'a, Self> {
        Ref::Borrowed(<Self as Primitive>::deref_blob(blob))
    }

    #[inline]
    fn deref_blob<'a, P: Ptr>(blob: FullyValidBlob<'a, Self, P>) -> &'a Self {
        match <Self as Primitive>::validate_blob(Blob::from(blob)) {
            Ok(_) => unsafe { blob.assume_valid() },
            Err(e) => panic!("fully valid offset not valid: {:?}", e),
        }
    }
}

impl Default for OffsetMut<'static, '_> {
    fn default() -> Self {
        Offset::new(0).unwrap().into()
    }
}

impl<'p, 'v> Ptr for OffsetMut<'p, 'v> {
    type Persist = Offset<'p, 'v>;
    type Zone = PileMut<'p, 'v>;
    type Allocator = PileMut<'p, 'v>;

    #[inline]
    fn allocator() -> Self::Allocator
        where Self: Default
    {
        let pile = PileMut::default();

        // Safe because OffsetMut::default() is implemented for the exact same lifetimes as
        // PileMut::default()
        unsafe { mem::transmute(pile) }
    }

    #[inline]
    fn clone_ptr<T: Clone>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        let raw = match Self::try_get_dirty(ptr) {
            Ok(r) => {
                let cloned = ManuallyDrop::new(r.clone());

                unsafe { Self::alloc(&cloned) }
            },
            Err(offset) => offset.into(),
        };

        unsafe {
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(
                    FatPtr { raw, metadata: ptr.metadata }
            ))
        }
    }

    #[inline]
    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            // Safe because drop_take_unsized() takes a FnOnce, so this closure can only run once.
            unsafe { ManuallyDrop::drop(value) }
        )
    }

    #[inline]
    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        let FatPtr { raw, metadata } = owned.into_inner().into_inner();

        match raw.kind() {
            Kind::Offset(_) => {},
            Kind::Ptr(ptr) => {
                unsafe {
                    let r: &mut T = &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                    let r: &mut ManuallyDrop<T> = &mut *(r as *mut _ as *mut _);

                    f(r);

                    let layout = fix_layout(Layout::for_value(r));
                    if layout.size() > 0 {
                        std::alloc::dealloc(r as *mut _ as *mut u8, layout);
                    }
                }
            }
        }
    }

    #[inline]
    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, Self::Persist> {
        match ptr.raw.kind() {
            Kind::Offset(offset) => Err(offset),
            Kind::Ptr(nonnull) => {
                unsafe {
                    Ok(&*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'p,'v> {
    Offset(Offset<'p,'v>),
    Ptr(NonNull<u16>),
}

#[inline]
fn fix_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

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

    #[inline]
    pub(super) unsafe fn alloc<T: ?Sized + Pointee>(src: &ManuallyDrop<T>) -> Self {
        let layout = fix_layout(Layout::for_value(src));

        let ptr = if layout.size() > 0 {
            let dst = NonNull::new(std::alloc::alloc(layout))
                              .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

            ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                layout.size());

            dst.cast()
        } else {
            NonNull::new_unchecked(layout.align() as *mut u16)
        };

        Self::from_ptr(ptr)
    }

    #[inline]
    pub(super) unsafe fn try_take<T: ?Sized + Pointee + Owned>(self, metadata: T::Metadata) -> Result<T::Owned, Offset<'s,'m>> {
        let this = ManuallyDrop::new(self);

        match this.kind() {
            Kind::Offset(offset) => Err(offset),
            Kind::Ptr(ptr) => {
                let ptr: *mut T = T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                let r = &mut *(ptr as *mut ManuallyDrop<T>);
                let layout = fix_layout(Layout::for_value(r));

                let owned = T::to_owned(r);

                if layout.size() > 0 {
                    std::alloc::dealloc(r as *mut _ as *mut u8, layout);
                }

                Ok(owned)
            }
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
