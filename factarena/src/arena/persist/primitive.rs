use super::*;

impl<A: Arena> Type<A> for ! {
    type Error = !;
    type RefOwned = !;

    fn store_blob<'a>(&self, _arena: &mut impl AllocBlob<A>) -> Own<Self, A> {
        match *self {}
    }
}

unsafe impl<A: Arena> Persist<A> for ! {
    type Error = !;

    #[inline]
    fn verify<'a>(_: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        panic!("Persist::<{}>::verify() called for !",
               type_name::<A>())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateBoolError(pub u8);

unsafe impl<A: Arena> Persist<A> for bool {
    type Error = ValidateBoolError;

    #[inline]
    fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        match unver[0] {
            1 | 0 => Ok(unsafe { unver.assume_init() }),
            x => Err(ValidateBoolError(x)),
        }
    }
}

impl<A: Arena> Type<A> for bool {
    type Error = !;
    type RefOwned = Self;

    fn store_blob<'a>(&self, arena: &mut impl AllocBlob<A>) -> Own<Self, A>
        where A: BlobArena
    {
        arena.alloc_blob(self)
    }
}

macro_rules! impl_all_valid {
    ( $( $t:ty, )* ) => {
        $(
            unsafe impl<A: Arena> Persist<A> for $t {
                type Error = !;

                #[inline(always)]
                fn verify<'a>(unver: Unverified<'a, Self>, _: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
                    Ok(unsafe { unver.assume_init() })
                }
            }

            impl<A: Arena> Type<A> for $t {
                type Error = !;
                type RefOwned = Self;

                fn store_blob<'a>(&self, arena: &mut impl AllocBlob<A>) -> Own<Self, A>
                    where A: BlobArena
                {
                    arena.alloc_blob(self)
                }
            }
        )*
    }
}

impl_all_valid! {
    (),
    u8, i8,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
