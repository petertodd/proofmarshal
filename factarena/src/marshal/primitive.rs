use super::*;

unsafe impl Load for ! {
    type Owned = !;
    type Error = !;

    fn validate_blob<'a, A: VerifyPtr>(_: BlobRef<'a, Self>, _: A) -> Result<&'a Self, Self::Error> {
        unreachable!("can't Load a !")
    }
}

impl<A> Store<A> for ! {
    fn store_owned<D: Emplace<Self>>(owned: Self::Owned, _: D) -> D::Done {
        match owned {}
    }

    fn store_blob<S: StorePtr<A>>(&self, _: S) -> Result<S::Ptr, S::Error> {
        match *self {}
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateBoolError(pub u8);

unsafe impl Load for bool {
    type Owned = Self;
    type Error = ValidateBoolError;

    #[inline]
    fn validate_blob<'a, A>(blob: BlobRef<'a, Self>, _: A) -> Result<&'a Self, Self::Error> {
        match blob[0] {
            1 | 0 => Ok(unsafe { blob.assume_valid() }),
            x => Err(ValidateBoolError(x)),
        }
    }
}

impl<A> Store<A> for bool {
    fn store_owned<D: Emplace<Self>>(owned: Self::Owned, dst: D) -> D::Done {
        dst.emplace(owned)
    }

    fn store_blob<S: StorePtr<A>>(&self, _: S) -> Result<S::Ptr, S::Error> {
        unimplemented!()
    }
}

macro_rules! impl_all_valid {
    ( $( $t:ty, )* ) => {
        $(
            unsafe impl Load for $t {
                type Owned = Self;
                type Error = !;

                #[inline(always)]
                fn validate_blob<'a, A>(blob: BlobRef<'a, Self>, _: A) -> Result<&'a Self, Self::Error> {
                    Ok(unsafe { blob.assume_valid() })
                }
            }

            impl<A> Store<A> for $t {
                fn store_owned<D: Emplace<Self>>(owned: Self::Owned, dst: D) -> D::Done {
                    dst.emplace(owned)
                }

                fn store_blob<S: StorePtr<A>>(&self, _: S) -> Result<S::Ptr, S::Error> {
                    unimplemented!()
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
