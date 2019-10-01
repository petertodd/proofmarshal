use super::*;

macro_rules! impl_primitives {
    ($( $t:ty, )+) => {
        $(
            impl<P> Coerced<P> for $t {
                type Coerced = $t;
            }

            impl<P> Type<P> for $t {}
        )+
    }
}

impl_primitives! {
    !, (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}


impl<P, T: Type<P>> Coerce<P> for [T]
where <T as Coerce<P>>::Type: Sized + Clone,
{
    type Type = [T::Type];
    type Owned = Vec<T::Type>;
}

impl<P, T: Type<P>> Type<P> for [T]
where <T as Coerce<P>>::Type: Sized + Clone,
{
}
