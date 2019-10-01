use super::*;

/*
pub struct OptionError<T: Type<A>, A: Arena>(T::Error);

impl<A: Arena, T: Persist<A>> Type<A> for Option<T> {
    type Error = OptionLoadError<T,A>;

    type RefOwned = Option<T>;
}
*/

impl<A: Arena, T: Persist<A>> Value<A> for Option<T> {
    type Primitives = Option<T>;
}

impl<A: Arena, T: Persist<A>> Persist<A> for Option<T> {
    type Error = fmt::Debug;

    fn verify<'a>(unver: Unverified<'a, Self>, loader: &mut impl VerifyPtr<A>)
        -> Result<Valid<'a, Self>, Self::Error>;
}
