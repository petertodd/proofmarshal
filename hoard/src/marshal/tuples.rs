use super::*;

#[derive(Debug)]
pub struct LoadTupleError;

impl<Z: Zone, A: Marshal<Z>, B: Marshal<Z>> Marshal<Z> for (A,B) {
    type Error = LoadTupleError;

    #[inline(always)]
    fn pile_layout() -> pile::Layout where Z: pile::Pile {
        A::pile_layout().extend(B::pile_layout())
    }

    #[inline(always)]
    fn pile_load<'p>(blob: Blob<'p, Self, Z>, pile: &Z) -> Result<Cow<'p, Self>, Self::Error>
        where Z: pile::Pile
    {
        unimplemented!()
    }

    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        let mut buf = vec![0; Self::pile_layout().size()];
        StructDumper::new(dumper, &mut buf)
                     .dump_value(&self.0)?
                     .dump_value(&self.1)?
                     .done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!((1u8, 0xaabb_u16).pile_store(vec![]).unwrap(),
                   &[1, 0xbb, 0xaa]);
    }
}
