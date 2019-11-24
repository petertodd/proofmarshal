use core::any::Any;
use core::fmt;
use core::mem;
use core::marker::PhantomData;
use core::ops;
use core::slice::SliceIndex;
use core::slice;

use std::sync::Arc;

use super::Offset;

#[derive(Debug, Clone)]
pub struct Snapshot<'p, M: ?Sized = dyn Mapping> {
    marker: PhantomData<&'p mut ()>,
    slice_ptr: *const u8,
    slice_len: usize,

    mapping: M,
}

pub unsafe trait Mapping : fmt::Debug + Any + Sync {
    fn as_bytes(&self) -> &[u8];
}

unsafe impl Mapping for &'static [u8] {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

unsafe impl Mapping for Vec<u8> {
    fn as_bytes(&self) -> &[u8] {
        &self[..]
    }
}

unsafe impl<M: Mapping + Send> Mapping for Arc<M> {
    fn as_bytes(&self) -> &[u8] {
        (**self).as_bytes()
    }
}

unsafe impl<M: Sync> Sync for Snapshot<'_, M> {}

pub static EMPTY_SNAPSHOT: Snapshot<&'static [u8]> =
    Snapshot {
	marker: PhantomData,
	slice_ptr: 1 as *const u8,
	slice_len: 0,
	mapping: &[],
    };


impl<'m, M: Mapping> Snapshot<'m, M> {
    pub unsafe fn new_unchecked(mapping: M) -> Self {
        Self::new_unchecked_with_range(mapping, ..).unwrap()
    }

    pub unsafe fn new_unchecked_with_range(mapping: M, range: impl SliceIndex<[u8], Output=[u8]>) -> Option<Self>
    {
        // Remember that as_ref() doesn't necessarily have to return the same slice each time, so
        // we have to be careful to call as_ref() exactly once.
        if let Some(slice) = mapping.as_bytes().get(range) {
            Some(Self {
                marker: PhantomData,
                slice_ptr: slice.as_ptr(),
                slice_len: slice.len(),
                mapping,
            })
        } else {
            None
        }
    }
}

impl<'m, M: ?Sized> Snapshot<'m, M> {
    pub fn truncate(&mut self, len: usize) {
        if len < self.slice_len {
            self.slice_len = len;
        }
    }
}

impl<M: ?Sized> ops::Deref for Snapshot<'_, M> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.slice_ptr, self.slice_len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mapping = vec![0u8;0];
        let snapshot = unsafe { Snapshot::new_unchecked(mapping) };

        let dyn_snap: &Snapshot = &snapshot;

        assert_eq!(dyn_snap.len(), 0);
    }
}
