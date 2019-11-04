use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use persist::Le;

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Offset<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    offset: Le<NonZeroU64>,
}

impl<'h> Offset<'h> {
    pub fn new(offset: usize) -> Option<Self> {
        match NonZeroU64::new(offset as u64) {
            None => None,
            Some(offset) => Some(
                Self {
                    marker: PhantomData,
                    offset: offset.into(),
                }
            )
        }
    }

    pub fn get(self) -> usize {
        self.offset.get().get() as usize
    }
}
