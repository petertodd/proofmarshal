use crate::prelude::*;

use crate::arena::persist::*;
use crate::arena::*;

#[repr(C)]
#[derive(Debug)]
pub struct RawLinkedList<T: Type<A>, A: Arena = Heap> {
    tip: Option<Own<Cell<T,A>, A>>,
}

impl<T: Type<A>, A: Locate> RawLinkedList<T,A> {
    pub fn push(&mut self, mut arena: impl Alloc<Arena=A>, value: T)
        where A: Alloc
    {
        let next_cell = Cell {
            value: arena.alloc(value),
            next: RawLinkedList { tip: self.tip.take() },
        };

        self.tip = Some(arena.alloc(next_cell));
    }

    pub fn try_first<'a>(&'a mut self, ar: impl TryGet<A>) -> Result<Option<&'a Own<T,A>>, A::Error> {
        match &self.tip {
            None => Ok(None),
            Some(own) => {
                let cell = ar.try_get(own)?;

                unimplemented!()
            }
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Cell<T: Type<A>, A: Arena = Heap> {
    value: Own<T,A>,
    next: RawLinkedList<T,A>,
}

impl<T: Type<A>, A: Arena> Default for RawLinkedList<T,A> {
    fn default() -> Self {
        RawLinkedList { tip: None }
    }
}

impl<T: Type<A>, A: Arena> Type<A> for Cell<T,A> {
    type Error = !;
    type RefOwned = Self;

    fn store_blob<'a>(&self, _arena: &mut impl AllocBlob<A>) -> Own<Self, A>
        where A: BlobArena
    {
        unimplemented!()
    }
}

unsafe impl<T: Type<A>, A: BlobArena> Persist<A> for Cell<T,A> {
    type Error = !;

    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        unver.verify_struct(arena)
             .field::<Own<T,A>>().unwrap()
             .field::<RawLinkedList<T,A>>().unwrap()
             .finish()
    }
}

impl<T: Type<A>, A: Arena> Type<A> for RawLinkedList<T,A> {
    type Error = !;
    type RefOwned = Self;

    fn store_blob<'a>(&self, _arena: &mut impl AllocBlob<A>) -> Own<Self, A>
        where A: BlobArena
    {
        unimplemented!()
    }
}

unsafe impl<T: Type<A>, A: BlobArena> Persist<A> for RawLinkedList<T,A> {
    type Error = !;

    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        unver.verify_struct(arena)
             .field::<Option<Own<T,A>>>().unwrap()
             .finish()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut l = RawLinkedList::<u8>::default();

        for i in 0 .. 100 {
            l.push(&mut Heap, i);
        }
    }
}
