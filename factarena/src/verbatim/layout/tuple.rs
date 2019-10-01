use core::marker::PhantomData;

use super::*;

/// A tuple
#[derive(Default)]
pub struct Tuple<I = ()>(I);

impl<P,I> Value<P> for Tuple<I>
where I: Value<P>
{
    type Value = Tuple<I::Value>;
}

pub struct Item<V,N = ()> {
    item: V,
    next: N,
}

impl<P,V,N> Value<P> for Item<V,N>
where V: Value<P>,
      N: Value<P>,
{
    type Value = Item<V::Value, N::Value>;
}

impl<N> Tuple<N> {
    pub fn push_front<V>(self, item: V) -> Tuple<Item<V,N>> {
        Tuple(Item {
            item,
            next: self.0,
        })
    }
}

pub trait TupleLayout {
    type Item : Layout;
    type Next : TupleLayout;

    fn len(&self, ptr: &impl Layout) -> usize;

    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        self.get()
            .and_then(|(item, next)|
                item.nonzero_niche(ptr)
                    .or_else(|| next.nonzero_niche(ptr))
            )
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool;

    fn get(&self) -> Option<(&Self::Item, &Self::Next)>;
}

impl<I: TupleLayout> Layout for Tuple<I> {
    fn len(&self, ptr: &impl Layout) -> usize {
        self.0.len(ptr)
    }

    fn nonzero_niche(&self, ptr: &impl Layout) -> Option<Range<usize>> {
        self.0.nonzero_niche(ptr)
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.0.inhabited(ptr)
    }
}

impl TupleLayout for ! {
    type Item = !;
    type Next = !;

    fn len(&self, _ptr: &impl Layout) -> usize { 0 }
    fn get(&self) -> Option<(&Self::Item, &Self::Next)> { match *self {} }
    fn inhabited(&self, _: &impl Layout) -> bool { match *self {} }
}

impl TupleLayout for () {
    type Item = !;
    type Next = !;

    fn len(&self, _ptr: &impl Layout) -> usize {
        0
    }

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> {
        None
    }

    fn inhabited(&self, _: &impl Layout) -> bool {
        true
    }
}

impl<T,N> TupleLayout for Item<T,N>
where T: Layout,
      N: TupleLayout,
{
    type Item = T;
    type Next = N;

    fn len(&self, ptr: &impl Layout) -> usize {
        self.item.len(ptr) + self.next.len(ptr)
    }

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> {
        Some((&self.item, &self.next))
    }

    fn inhabited(&self, ptr: &impl Layout) -> bool {
        self.item.inhabited(ptr) && self.next.inhabited(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::num::NonZeroU8;

    #[test]
    fn test() {
        let ptr = &Primitive::<()>::default();
        let l = Tuple(());

        assert_eq!(l.len(ptr), 0);
        assert_eq!(l.nonzero_niche(ptr), None);

        let l = Tuple(()).push_front(Primitive::<u8>::default());
        assert_eq!(l.len(ptr), 1);
        assert_eq!(l.nonzero_niche(ptr), None);

        let l = Tuple(()).push_front(Primitive::<NonZeroU8>::default());
        assert_eq!(l.len(ptr), 1);
        assert_eq!(l.nonzero_niche(ptr), Some(0 .. 1));
    }
}
