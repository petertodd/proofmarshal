//! Volatile, in-memory, zone allocation.

use std::alloc::Layout;
use std::cmp;
use std::ptr::NonNull;

use owned::Take;

use crate::{
    pointee::Pointee,
    ptr::*,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap(NonNull<u16>);

#[inline]
fn min_align_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl Ptr for Heap {
    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            let layout = min_align_layout(Layout::for_value(src));

            let ptr = if layout.size() > 0 {
                let dst = NonNull::new(std::alloc::alloc(layout))
                    .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

                std::ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                              layout.size());

                dst.cast()
            } else {
                NonNull::new_unchecked(layout.align() as *mut u16)
            };

            Bag::new_unchecked(Fat {
                raw: Heap(ptr),
                metadata,
            })
        })
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        let value = &mut *T::make_fat_ptr_mut(self.0.cast().as_ptr(), metadata);
        let layout = Layout::for_value(value);

        std::ptr::drop_in_place(value);
        std::alloc::dealloc(self.0.cast().as_ptr(), layout);
    }

    unsafe fn clone_unchecked<T: Clone>(&self) -> Self {
        todo!()
    }

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Option<&T> {
        Some(&mut *T::make_fat_ptr_mut(self.0.cast().as_ptr(), metadata))
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self(NonNull::new(1 as *mut u16).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let bag = Heap::alloc(123u8);
    }
}
