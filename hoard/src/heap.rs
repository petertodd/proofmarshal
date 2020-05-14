//! Volatile, in-memory, zone allocation.

use std::alloc::Layout;
use std::borrow::Borrow;
use std::cmp;
use std::ptr::NonNull;

use owned::Take;

use crate::{
    pointee::Pointee,
    ptr::*,
    save::*,
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
    type Persist = !;

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

    unsafe fn try_get_dirty_unchecked<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<&T, !> {
        Ok(&mut *T::make_fat_ptr_mut(self.0.cast().as_ptr(), metadata))
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self(NonNull::new(1 as *mut u16).unwrap())
    }
}

impl AsPtr<Heap> for Heap {
    fn as_ptr(&self) -> &Self {
        self
    }
}

/*
impl<Q> Saved<Q> for Heap {
    type Saved = Q;
}

impl<'a, Q> Save<'a, Q> for Heap {
    type State = !;

    fn init_save_state(&'a self) -> Self::State {
        todo!()
    }

    fn poll<D: SavePtr<Q>>(&'a self, state: &mut Self::State, dst: D) -> Result<D, D::Error> {
        todo!()
    }

    fn encode<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }

    unsafe fn save_ptr<T: ?Sized + Pointee>(&'a self, metadata: T::Metadata) -> Result<Q, &'a T> {
        todo!()
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let bag = Heap::alloc(123u8);
    }
}
