use super::*;

use crate::tuple::Item;

unsafe impl<A, T: Persist<A>, N: Persist<A>> Persist<A> for Item<T,N> {
    type Error = ItemError<T,N,A>;

    fn validate_bytes<'a>(unver: MaybeValid<'a, Self, [u8]>, arena: &A) -> Result<Valid<'a, Self, [u8]>, Self::Error> {
        unimplemented!()
    }

    fn write_canonical_bytes<W: io::Write>(&self, mut w: W) -> io::Result<W> {
        let w = self.0.write_canonical_bytes(w)?;
        self.1.write_canonical_bytes(w)
    }
}

pub enum ItemError<T: Persist<A>, N: Persist<A>, A> {
    Item(T::Error),
    Rest(N::Error),
}

impl<T: Persist<A>, N: Persist<A>, A> fmt::Debug for ItemError<T,N,A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ItemError::Item(err) => f.debug_tuple("ItemError::Item")
                                         .field(err)
                                         .finish(),
            ItemError::Rest(err) => f.debug_tuple("ItemError::Rest")
                                         .field(err)
                                         .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

}
