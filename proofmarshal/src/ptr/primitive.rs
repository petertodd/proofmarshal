use super::*;

macro_rules! impl_primitives {
    ($( $t:ty, )+) => {
        $(
            impl<P> Coerced<P> for $t {
                type Coerced = $t;
            }

            impl<P> Type<P> for $t {
                fn cast(coerced: &<Self as Coerce<P>>::Type) -> Self {
                    *coerced
                }
            }
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

macro_rules! array_impls {
    ($($N:literal)+) => {
        $(
            impl<P, T: Coerce<P>> Coerced<P> for [T;$N]
            {
                type Coerced = [T::Owned;$N];
            }
            impl<P, T: Type<P>> Type<P> for [T;$N] {
                fn cast(coerced: &Self::Type) -> Self
                {
                    let a: &T::Owned = &coerced[0];
                    unimplemented!()
                }
            }
        )+
    }
}

array_impls! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}
