use core::marker::PhantomData;

use super::*;

/// A tuple
#[derive(Debug,Default)]
pub struct Struct<F,S> {
    pub name: S,
    pub fields: F,
}

impl<F,S> Layout for Struct<F,S>
where F: TupleLayout,
{
    fn len(&self, ptr: &impl Layout) -> usize {
        self.fields.len(ptr)
    }

    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        self.fields.nonzero_niche(ptr)
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.fields.inhabited(ptr)
    }
}

pub struct Field<S,V,N = ()> {
    pub name: S,
    pub item: V,
    pub next: N,
}

pub trait FieldLayout<S> : TupleLayout {
    type NextField : FieldLayout<S>;

    fn get_field(&self) -> Option<Field<&S, &Self::Item, &Self::NextField>>;
}

impl<S> FieldLayout<S> for ! {
    type NextField = !;

    fn get_field(&self) -> Option<Field<&S, &Self::Item, &Self::NextField>> {
        match *self {}
    }
}

impl<S> FieldLayout<S> for () {
    type NextField = !;

    fn get_field(&self) -> Option<Field<&S, &Self::Item, &Self::NextField>> {
        None
    }
}

impl<S,T,N> TupleLayout for Field<S,T,N>
where T: Layout,
      N: TupleLayout,
{
    type Item = T;
    type Next = N;

    fn len(&self, ptr: &impl Layout) -> usize {
        self.item.len(ptr) + self.next.len(ptr)
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.item.inhabited(ptr) && self.next.inhabited(ptr)
    }

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> {
        Some((&self.item, &self.next))
    }
}

impl<S,T,N> FieldLayout<S> for Field<S,T,N>
where T: Layout,
      N: FieldLayout<S>,
{
    type NextField = N;

    fn get_field(&self) -> Option<Field<&S, &Self::Item, &Self::NextField>> {
        Some(Field {
            name: &self.name,
            item: &self.item,
            next: &self.next,
        })
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
