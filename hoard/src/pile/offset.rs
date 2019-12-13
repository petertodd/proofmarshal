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
//! 1) Making `Pile<'s, 'p>` and `Offset<'s, 'p>` *invariant* over `'p`.
//! 2) Ensuring via the `Unique` API that exactly one pile can exist for a given `'p` lifetime.
//!
//! This means that the following won't even compile because `Pile<'s, 'static>` and `Pile<'s, 'p>`
//! are incompatible types:
//!
//! ```compile_fail
//! # use hoard::pile::{Pile, Offset};
//! fn foo<'s, 'p>(pile: Pile<'s, 'p>, offset: Offset<'s, 'p>) {}
//!
//! fn bar<'s, 'p>(pile: Pile<'s, 'static>, offset: Offset<'s, 'p>) {
//!     foo(pile, offset)
//! }
//! ```
//! similarly `Offset<'s, 'static>` and `Offset<'s, 'p>` are incompatible:
//!
//! ```compile_fail
//! # use hoard::pile::{Pile, Offset};
//! # fn foo<'s, 'p>(pile: Pile<'s, 'p>, offset: Offset<'s, 'p>) {}
//! fn bar<'s, 'p>(pile: Pile<'s, 'p>, offset: Offset<'s, 'static>) {
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
//! Note the lifetimes! It's the pointer's job to "own" that data, so `Offset<'s, 'p>` owns a
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
pub struct Offset<'s, 'p> {
    marker: PhantomData<(
        fn(Pile<'s, 'p>) -> &'s (),
        &'p [u8],
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

impl<'s,'p> Offset<'s,'p> {
    pub const MAX: usize = (1 << 62) - 1;

    pub fn new(offset: usize) -> Option<Self> {
        let offset = offset as u64;
        offset.checked_shl(1).map(|offset|
            Self {
                marker: PhantomData,
                raw: NonZeroU64::new(offset | 1).unwrap().into(),
            }
        )
    }

    pub fn to_static(&self) -> Offset<'static, 'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    pub fn get(&self) -> usize {
        (self.raw.get().get() >> 1) as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'s,'p>(Offset<'s,'p>);
unsafe impl Persist for OffsetMut<'_,'_> {}

unsafe impl<'s,'p> TryCastRef<OffsetMut<'s,'p>> for Offset<'s,'p> {
    type Error = !;

    #[inline(always)]
    fn try_cast_ref(&self) -> Result<&OffsetMut<'s,'p>, Self::Error> {
        Ok(unsafe { mem::transmute(self) })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TryCastOffsetMutError(());

unsafe impl<'s,'p> TryCastRef<Offset<'s,'p>> for OffsetMut<'s,'p> {
    type Error = TryCastOffsetMutError;

    #[inline]
    fn try_cast_ref(&self) -> Result<&Offset<'s,'p>, Self::Error> {
        match self.kind() {
            Kind::Offset(_) => Ok(&self.0),
            Kind::Ptr(_) => Err(TryCastOffsetMutError(())),
        }
    }
}

impl<'s, 'p> From<Offset<'s,'p>> for OffsetMut<'s,'p> {
    fn from(offset: Offset<'s,'p>) -> Self {
        Self(offset)
    }
}

impl<'s,'p> Ptr for Offset<'s, 'p> {
    type Persist = Self;
    type Zone = Pile<'s, 'p>;
    type Allocator = crate::never::NeverAllocator<Self>;

    fn allocator() -> Self::Allocator {
        panic!()
    }

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

    fn dealloc_owned<T: ?Sized + Pointee>(own: OwnedPtr<T, Self>) {
        let _ = own.into_inner();
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(_: OwnedPtr<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
    }

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

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().get().to_le_bytes())?
           .finish()
    }

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

    fn decode_blob<'a, P: Ptr>(blob: FullyValidBlob<'a, Self, P>) -> Self {
        <Self as Primitive>::deref_blob(blob).clone()
    }

    fn load_blob<'a, P: Ptr>(blob: FullyValidBlob<'a, Self, P>) -> Ref<'a, Self> {
        Ref::Borrowed(<Self as Primitive>::deref_blob(blob))
    }

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

impl<'s, 'p> Ptr for OffsetMut<'s, 'p> {
    type Persist = Offset<'s, 'p>;
    type Zone = PileMut<'s, 'p>;
    type Allocator = PileMut<'s, 'p>;

    fn allocator() -> Self::Allocator
        where Self: Default
    {
        let pile = PileMut::default();

        // Safe because OffsetMut::default() is implemented for the exact same lifetimes as
        // PileMut::default()
        unsafe { mem::transmute(pile) }
    }

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

    fn dealloc_owned<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            unsafe {
                core::ptr::drop_in_place(value)
            }
        )
    }

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
pub enum Kind<'s,'p> {
    Offset(Offset<'s,'p>),
    Ptr(NonNull<u16>),
}

fn fix_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl<'s,'m> OffsetMut<'s,'m> {
    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

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
