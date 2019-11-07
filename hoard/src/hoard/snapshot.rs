use core::cmp;
use core::convert::TryFrom;
use core::fmt;
use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use pointee::Pointee;

use super::*;

use crate::{Zone, Alloc, Ptr, Rec, Load, Store};

/// Read-only snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    pub(crate) mapping: Arc<Mmap>,
}

impl<'h> Snapshot<'h> {
    pub(crate) fn from_mapping(mapping: Arc<Mmap>) -> Self {
        Self {
            marker: PhantomData,
            mapping,
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.mapping[mem::size_of::<Header>() - mem::size_of::<Word>() .. ]
    }
}

/*
impl<'h> crate::Zone for Snapshot<'h> {
    type Ptr = Offset;
    type Allocator = Self;
    type Error = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T, Self>) {
        unimplemented!()
    }
}

impl<'a, 'h> Alloc for Snapshot<'h> {
    type Zone = Snapshot<'h>;

    fn alloc<T: Store<Self::Zone>>(&mut self, value: T) -> Rec<T, Self::Zone> {
        unimplemented!()
    }

    fn zone(&self) -> Self::Zone {
        self
    }
}
*/
