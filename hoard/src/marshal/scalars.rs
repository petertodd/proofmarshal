use super::*;

impl<Z: Zone> Marshal<Z> for () {
    type Error = !;

    #[inline(always)]
    fn pile_layout() -> pile::Layout where Z: pile::Pile {
        pile::Layout::new(0)
    }

    #[inline(always)]
    fn pile_load<'p>(blob: Blob<'p, Self, Z>, _: &Z) -> Result<Ref<'p, Self, Z>, Self::Error>
        where Z: pile::Pile
    {
        Ok(Ref::Borrowed(unsafe { blob.assume_valid() }))
    }

    #[inline(always)]
    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        dumper.dump_blob(&[])
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LoadBoolError(u8);

impl<Z: Zone> Marshal<Z> for bool {
    type Error = LoadBoolError;

    #[inline(always)]
    fn pile_layout() -> pile::Layout where Z: pile::Pile {
        pile::Layout::new(1)
    }

    #[inline(always)]
    fn pile_load<'p>(blob: Blob<'p, Self, Z>, _: &Z) -> Result<Ref<'p, Self, Z>, Self::Error>
        where Z: pile::Pile
    {
        match blob[0] {
            0 | 1 => Ok(Ref::Borrowed(unsafe { blob.assume_valid() })),
            x => Err(LoadBoolError(x)),
        }
    }

    #[inline(always)]
    fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
        where Z: pile::Pile
    {
        dumper.dump_blob(&[*self as u8])
    }
}


macro_rules! impl_ints {
    ($($t:ty,)+) => {
        $(
            impl<Z: Zone> Marshal<Z> for $t {
                type Error = !;

                #[inline(always)]
                fn pile_layout() -> pile::Layout where Z: pile::Pile {
                    pile::Layout::new(mem::size_of::<Self>())
                }

                #[inline(always)]
                fn pile_load<'p>(blob: Blob<'p, Self, Z>, _: &Z) -> Result<Ref<'p, Self, Z>, Self::Error>
                    where Z: pile::Pile
                {
                    Ok(Ref::Borrowed(unsafe { blob.assume_valid() }))
                }

                #[inline(always)]
                fn pile_store<D: pile::Dumper<Pile=Z>>(&self, dumper: D) -> Result<D::Done, D::Error>
                    where Z: pile::Pile
                {
                    dumper.dump_blob(&self.to_le_bytes())
                }
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}
