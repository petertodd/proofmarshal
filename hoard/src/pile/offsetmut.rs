//! Copy-on-write pile offsets.

use core::alloc::Layout;
use core::cmp;
use core::fmt;
use core::mem::{self, ManuallyDrop};
use core::ptr::NonNull;

use thiserror::Error;

use nonzero::NonZero;
use owned::{Take, IntoOwned};

use crate::coerce::TryCoerce;
use crate::pointee::Pointee;
use crate::marshal::blob;
use crate::marshal::decode::*;
use crate::marshal::PtrValidator;
use crate::zone::*;

use super::offset::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p,'v>(Offset<'p,'v>);
unsafe impl NonZero for OffsetMut<'_,'_> {}

unsafe impl<'p, 'v> TryCoerce<OffsetMut<'p, 'v>> for Offset<'_, '_> {
    type Error = !;
}

unsafe impl<'p, 'v> TryCoerce<Offset<'p, 'v>> for OffsetMut<'_, '_> {
    type Error = TryCoerceOffsetMutError;

    #[inline(always)]
    fn try_coerce_ptr(this: &Self) -> Result<*const Offset<'p,'v>, Self::Error> {
        match this.kind() {
            Kind::Offset(_) => Ok(this as *const _ as *const _),
            Kind::Ptr(ptr) => Err(TryCoerceOffsetMutError { ptr }),
        }
    }
}

/// Returned if an `OffsetMut` can't be coerced to an `Offset` due to being a pointer.
#[derive(Error, Debug, PartialEq, Eq, Hash)]
#[error("OffsetMut is a pointer: {ptr:?}")]
pub struct TryCoerceOffsetMutError {
    pub ptr: NonNull<u16>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'p,'v> {
    Offset(Offset<'p,'v>),
    Ptr(NonNull<u16>),
}

impl<'p, 'v> From<Offset<'p,'v>> for OffsetMut<'p,'v> {
    #[inline]
    fn from(offset: Offset<'p,'v>) -> Self {
        Self(offset)
    }
}

unsafe impl Persist for OffsetMut<'_, '_> {
    type Persist = Offset<'static, 'static>;
    type Error = ValidateOffsetError;
}

unsafe impl<'a, Z> ValidateChildren<'a, Z> for OffsetMut<'_, '_> {
    type State = ();

    #[inline(always)]
    fn validate_children(_: &Offset<'static, 'static>) -> () {}

    #[inline(always)]
    fn poll<V: PtrValidator<Z>>(this: &Self::Persist, _: &mut (), _: &V) -> Result<(), V::Error> {
        Ok(())
    }
}
impl<Z> Decode<Z> for OffsetMut<'_,'_> {}

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl<'p,'v> OffsetMut<'p,'v> {
    pub fn alloc<T: ?Sized + Pointee, Z>(src: impl Take<T>) -> OwnedPtr<T, Z>
        where Z: Zone<Ptr=Self>
    {
	src.take_unsized(|src| unsafe {
	    let metadata = T::metadata(src);
	    let layout = min_align_layout(Layout::for_value(src));

	    let ptr = if layout.size() > 0 {
		let dst = NonNull::new(std::alloc::alloc(layout))
				  .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

		core::ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                               layout.size());

		dst.cast()
	    } else {
		NonNull::new_unchecked(layout.align() as *mut u16)
	    };

            let fatptr = FatPtr {
                raw: Self::from_ptr(ptr),
                metadata,
            };
	    OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr))
	})
    }

    pub fn try_take_dirty_unsized<T: ?Sized + Pointee, Z, R>(
        owned: OwnedPtr<T, Z>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, Offset<'p,'v>>) -> R
    ) -> R
    where Z: Zone<Ptr=Self>
    {
        let FatPtr { raw, metadata } = owned.into_inner().into_inner();

        match raw.kind() {
            Kind::Ptr(nonnull) => unsafe {
                let v: &mut T = &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), metadata);
                let v: &mut ManuallyDrop<T> = &mut *(v as *mut _ as *mut _);

                struct DeallocOnDrop {
                    layout: Layout,
                    ptr: *mut u8,
                }

                impl Drop for DeallocOnDrop {
                    #[inline(always)]
                    fn drop(&mut self) {
                        if self.layout.size() > 0 {
                            unsafe { std::alloc::dealloc(self.ptr, self.layout) }
                        }
                    }
                }
                let dealloc_on_drop = DeallocOnDrop {
                    layout: min_align_layout(Layout::for_value(v)),
                    ptr: v as *mut _ as *mut u8,
                };

                let r = f(Ok(v));

                drop(dealloc_on_drop);

                r
            },
            Kind::Offset(offset) => f(Err(offset)),
        }
    }

    #[inline]
    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        debug_assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

    #[inline]
    pub fn kind(&self) -> Kind<'p,'v> {
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
    pub fn get_offset(&self) -> Option<Offset<'p,'v>> {
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
