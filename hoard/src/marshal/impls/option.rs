use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct LoadOptionError;

impl<Z: Zone, T: Marshal<Z>> Marshal<Z> for Option<T> {
    type Error = LoadOptionError;

    fn pile_layout() -> pile::Layout
        where Z: pile::Pile
    {
        todo!()
    }

    fn pile_load<'p>(blob: Blob<'p, Self, Z>, pile: &Z) -> Result<Ref<'p, Self, Z>, Self::Error>
        where Z: pile::Pile
    {
        todo!()
    }

    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        todo!()
    }
}
