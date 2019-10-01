use super::*;

unsafe impl<T: ?Sized + Type<A>, A: BlobArena> Persist<A> for Own<T,A>
{
    type Error = Error<T,A>;

    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>) -> Result<&'a Self, Self::Error> {
        let this: Result<&Self,!> = unver.verify_struct(&())
                                         .field::<A::Ptr>().map_err(|err| Error::Ptr(err))?
                                         .field::<T::Metadata>().map_err(|err| Error::Metadata(err))?
                                         .finish();
        let this = this.unwrap();

        arena.verify_ptr::<T>(this.ptr(), this.metadata())
             .map_err(|err| Error::Value(err))?;

        Ok(this)
    }
}

pub enum Error<T: ?Sized + Type<A>, A: BlobArena> {
    Ptr(<A::Ptr as Persist>::Error),
    Metadata(<T::Metadata as Persist>::Error),
    Value(T::Error),
}

impl<T: ?Sized + Type<A>, A: BlobArena> fmt::Debug for Error<T,A> {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

unsafe impl<T: ?Sized + Type<A>, A: BlobArena> NonZero for Own<T,A> {}
