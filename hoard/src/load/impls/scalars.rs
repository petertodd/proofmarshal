use super::*;

macro_rules! impl_load_for_scalars {
    ($($t:ty,)+) => {$(
        impl Load for $t {
            type Blob = Self;
            type Zone = ();

            fn load(this: Self, _: &()) -> MaybeValid<Self> {
                this.into()
            }
        }
    )+}
}

impl_load_for_scalars! {
    (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

impl Load for ! {
    type Blob = Self;
    type Zone = ();
    fn load(this: Self, _: &()) -> MaybeValid<Self> {
        match this {}
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let foo: bool = <bool as Load>::load(true, &()).trust();
        assert!(foo);
    }
}
