use super::*;

impl<T: Type> Type for Option<T> {
    type Metadata = T::Metadata;
}

macro_rules! impl_type_for_primitives {
    ( $( $t:ty, )* ) => {
        $(
            impl Type for $t {
                type Metadata = ();
            }
        )*
    }
}

impl_type_for_primitives!{
    (),
    bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
