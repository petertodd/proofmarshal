use core::marker::PhantomData;
use core::num;

use super::*;

/// A singular primitive value
#[derive(Debug, Clone, Copy)]
pub struct Primitive<T> {
    marker: PhantomData<T>,
}

impl<T> Primitive<T> {
    pub const fn new() -> Self {
        Self { marker: PhantomData }
    }
}

impl<T> Default for Primitive<T> {
    fn default() -> Self {
        Primitive { marker: PhantomData }
    }
}

impl Layout for Primitive<()> {
    fn len(&self, _ptr: &impl Layout) -> usize {
        0
    }

    fn inhabited(&self, _ptr: &impl Layout) -> bool {
        true
    }
}

impl<P> Value<P> for Primitive<()> {
    type Value = ();
}

impl Layout for Primitive<u8> {
    fn len(&self, _ptr: &impl Layout) -> usize {
        1
    }
    fn inhabited(&self, _ptr: &impl Layout) -> bool {
        true
    }
}

impl<P> Value<P> for Primitive<u8> {
    type Value = u8;
}

impl Layout for Primitive<bool> {
    fn len(&self, _ptr: &impl Layout) -> usize {
        1
    }
    fn inhabited(&self, ptr: &impl Layout) -> bool {
        true
    }
}

impl<P> Value<P> for Primitive<bool> {
    type Value = bool;
}

impl Layout for Primitive<num::NonZeroU8> {
    fn len(&self, _ptr: &impl Layout) -> usize {
        1
    }

    fn nonzero_niche(&self, _ptr: &impl Layout) -> Option<Range<usize>> {
        Some(0 .. 1)
    }
    fn inhabited(&self, _ptr: &impl Layout) -> bool {
        true
    }
}

impl<P> Value<P> for Primitive<num::NonZeroU8> {
    type Value = num::NonZeroU8;
}

