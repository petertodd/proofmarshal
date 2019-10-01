use core::marker::PhantomData;
use core::cmp;

use super::*;

/// A tuple
#[derive(Debug,Default)]
pub struct Enum<V,S=&'static str> {
    pub name: S,
    pub variants: V,
}

pub trait VariantLayout<D> {
    type Match : Layout;
    type Next : VariantLayout<D>;

    fn num_variants(&self) -> usize;

    fn max_len(&self, ptr: &impl Layout) -> usize;
    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>>;
    fn num_inhabited_variants(&self, ptr: &impl Layout) -> usize;

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.num_inhabited_variants(ptr) > 0
    }
}

impl<V,S> Layout for Enum<V,S>
where V: VariantLayout<u8>
{
    fn len(&self, ptr: &impl Layout) -> usize {
        if self.variants.num_inhabited_variants(ptr) == 1 {
            self.variants.max_len(ptr)
        } else {
            1 + self.variants.max_len(ptr)
        }
    }

    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        if self.variants.num_inhabited_variants(ptr) == 1 {
            self.variants.nonzero_niche(ptr)
        } else {
            None
        }
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.variants.inhabited(ptr)
    }
}

impl<D> VariantLayout<D> for ! {
    type Match = !;
    type Next = !;

    fn num_variants(&self) -> usize { match *self {} }

    fn max_len(&self, _: &impl Layout) -> usize { match *self {} }
    fn nonzero_niche(&self, _: &impl Layout) -> Option<Range<usize>> { match *self {} }
    fn num_inhabited_variants(&self, _: &impl Layout) -> usize { match *self {} }
}

impl<D> VariantLayout<D> for () {
    type Match = !;
    type Next = !;

    fn max_len(&self, _: &impl Layout) -> usize { 0 }
    fn nonzero_niche(&self, _: &impl Layout) -> Option<Range<usize>> { None }
    fn num_variants(&self) -> usize { 0 }
    fn num_inhabited_variants(&self, _: &impl Layout) -> usize { 0 }
}

#[derive(Debug, Default)]
pub struct Variant<F,N=(),D=(),S=&'static str> {
    pub name: S,
    pub discriminant: D,
    pub fields: F,
    pub next: N,
}

impl<F,N,D,S> VariantLayout<u8> for Variant<F,N,D,S>
where F: Layout,
      N: VariantLayout<u8>,
{
    type Match = !;
    type Next = !;

    fn max_len(&self, ptr: &impl Layout) -> usize {
        if self.fields.inhabited(ptr) {
            cmp::max(self.fields.len(ptr), self.next.max_len(ptr))
        } else {
            self.next.max_len(ptr)
        }
    }
    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        if let Some(niche) = self.fields.nonzero_niche(ptr) {
            assert!(self.fields.inhabited(ptr));
            Some(niche)
        } else {
            self.next.nonzero_niche(ptr)
        }
    }

    fn num_variants(&self) -> usize {
        1 + self.next.num_variants()
    }

    fn num_inhabited_variants(&self, ptr: &impl Layout) -> usize {
        (if self.fields.inhabited(ptr) { 1 } else { 0 })
            + self.next.num_inhabited_variants(ptr)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    use core::num::NonZeroU8;

    #[test]
    fn test() {
    }
}
