//! Copy-on-write pile offsets.

use std::alloc::Layout;
use std::cmp;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ptr::NonNull;
use std::ops::Deref;

use thiserror::Error;

use owned::{Take, IntoOwned};

use crate::load::*;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p,'v>(Offset<'p,'v>);

impl<'p,'v> Ptr for OffsetMut<'p, 'v> {
    type Persist = Offset<'static, 'static>;

    unsafe fn dealloc<T: ?Sized + Pointee>(&self, metadata: T::Metadata) {
        todo!()
    }

    fn duplicate(&self) -> Self {
        *self
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        todo!()
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'p,'v> {
    Offset(Offset<'p,'v>),
    Ptr(NonNull<u16>),
}

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

/*
impl<'p,'v> Ptr for OffsetMut<'p, 'v> {
    type Persist = Offset<'static, 'static>;


    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        match self.kind() {
            Kind::Offset(_) => {},
            Kind::Ptr(ptr) => {
                let v = &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                let layout = min_align_layout(Layout::for_value(v));
                std::ptr::drop_in_place(v);

                if layout.size() > 0 {
                    std::alloc::dealloc(v as *mut _ as *mut _, layout);
                }
            },
        }
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        todo!()
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, Self::Persist> {
        match self.kind() {
            Kind::Ptr(ptr) => Ok(&*T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata)),
            Kind::Offset(ptr) => Err(ptr.cast()),
        }
    }
}

impl<'p, 'v> AsPtr<OffsetMut<'p, 'v>> for OffsetMut<'p, 'v> {
    fn as_ptr(&self) -> &Self {
        self
    }
}

impl<'p, 'v> AsPtr<OffsetMut<'p, 'v>> for Offset<'p, 'v> {
    fn as_ptr(&self) -> &OffsetMut<'p, 'v> {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<'p, 'v> AsPtr<OffsetMut<'p, 'v>> for crate::heap::Heap {
    fn as_ptr(&self) -> &OffsetMut<'p, 'v> {
        // FIXME: check 64-bitness
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<'p> Default for OffsetMut<'p, 'static> {
    fn default() -> Self {
        Offset::new(0).unwrap().into()
    }
}

impl<'p, 'v> From<Offset<'p,'v>> for OffsetMut<'p,'v> {
    #[inline]
    fn from(offset: Offset<'p,'v>) -> Self {
        Self(offset)
    }
}

impl<'p, 'v> Load for OffsetMut<'p, 'v> {
    type Error = super::offset::LoadOffsetError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}
*/

impl<'p,'v> OffsetMut<'p,'v> {
    /*
    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
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

            let fat = Fat {
                raw: Self::from_ptr(ptr),
                metadata,
            };

	    Bag::new_unchecked(fat)
	})
    }
    */

    /*
    pub fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        self, metadata: T::Metadata,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, Offset<'p,'v>>) -> R
    ) -> R
    {
        match self.kind() {
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
    */

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

impl ValidateBlob for OffsetMut<'_, '_> {
    type Error = super::offset::ValidateBlobOffsetError;

    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut blob = blob.validate_fields();
        blob.validate::<Offset>()?;
        unsafe { Ok(blob.assume_valid()) }
    }
}

impl<Z> Load<Z> for OffsetMut<'_, '_> {
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Self {
        blob.to_ref().clone()
    }

    fn load_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Ref<'a, Self> {
        blob.to_ref().into()
    }
}

unsafe impl Persist for OffsetMut<'_, '_> {
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
