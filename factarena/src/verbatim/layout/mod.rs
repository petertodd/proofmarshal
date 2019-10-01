use core::marker::PhantomData;
use core::ops::Range;

pub mod primitive;
pub mod tuple;
pub mod structs;
pub mod enums;

pub use self::primitive::Primitive;
pub use self::tuple::*;
pub use self::enums::*;

pub trait Layout {
    fn len(&self, ptr: &impl Layout) -> usize;

    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        None
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool;
}

impl Layout for ! {
    fn len(&self, _: &impl Layout) -> usize {
        match *self {}
    }

    fn inhabited(&self, _: &impl Layout) -> bool {
        match *self {}
    }
}

pub trait Value<P> {
    type Value;
}

impl<P> Value<P> for ! {
    type Value = !;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
