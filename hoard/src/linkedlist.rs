use super::*;

use marshal::{Marshal, Blob, StructDumper};

#[derive(Debug, Clone)]
pub struct LinkedList<T, Z: Zone> {
    tip: Option<Bag<Cell<T,Z>, Z>>,
    allocator: Z::Allocator,
}

impl<T, Z: Zone> LinkedList<T,Z> {
    pub fn new_in(allocator: Z::Allocator) -> Self {
        Self {
            tip: None,
            allocator,
        }
    }
}

impl<T, Z: Zone> LinkedList<T,Z>
where T: Marshal<Z>,
      Z: Marshal<Z>,
{
    pub fn push_front(&mut self, value: T) {
        let new_tip = Cell::new(value, self.tip.take());
        self.tip = Some(Bag::new_in(new_tip, &mut self.allocator));
    }

    pub fn pop_front(&mut self) -> Option<T>
        where Z: Take,
    {
        match self.tip.take() {
            None => None,
            Some(tip_bag) => {
                let tip = tip_bag.take();

                todo!()
            },
        }
    }
}

impl<T, Z: Zone, Y: Zone> Marshal<Y> for LinkedList<T,Z>
where T: Marshal<Y>,
      Z: Marshal<Y>,
      Z::Allocator: Marshal<Y>,
{
    type Error = LoadCellError;

    fn pile_layout() -> pile::Layout where Y: pile::Pile {
        todo!()
    }

    fn pile_load<'p>(blob: Blob<'p, Self, Y>, pile: &Y) -> Result<Ref<'p, Self, Y>, Self::Error>
        where Y: pile::Pile
    {
        todo!()
    }

    fn pile_store<D: pile::Dumper<Pile=Y>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Y: pile::Pile
    {
        self.tip.pile_store(dumper)
    }
}

#[derive(Debug)]
pub struct LoadLinkedListError;

#[derive(Debug, Clone)]
pub struct Cell<T, Z: Zone> {
    value: T,
    next: Option<Bag<Self, Z>>,
}

impl<T, Z: Zone> Cell<T,Z> {
    pub fn new(value: T, next: Option<Bag<Self, Z>>) -> Self {
        Self { value, next }
    }
}

#[derive(Debug)]
pub struct LoadCellError;

impl<T, Z: Zone, Y: Zone> Marshal<Y> for Cell<T,Z>
where T: Marshal<Y>,
      Z: Marshal<Y>,
{
    type Error = LoadCellError;

    fn pile_layout() -> pile::Layout where Y: pile::Pile {
        T::pile_layout().extend(<Option<Bag<Self, Z>> as Marshal<Y>>::pile_layout())
    }

    fn pile_load<'p>(blob: Blob<'p, Self, Y>, pile: &Y) -> Result<Ref<'p, Self, Y>, Self::Error>
        where Y: pile::Pile
    {
        todo!()
    }

    fn pile_store<D: pile::Dumper<Pile=Y>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Y: pile::Pile
    {
        let dst = vec![0; Self::pile_layout().size()];
        StructDumper::new(dumper, dst)
                     .dump_value(&self.value)?
                     .dump_value(&self.next)?
                     .done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use heap::Heap;

    #[test]
    fn test() {
        let cell = Cell::<u8,Heap>::new(10u8, None);
    }
}
