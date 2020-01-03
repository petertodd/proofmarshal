use super::*;

macro_rules! peel {
    ($name:ident, $( $rest_name:ident,)* ) => (tuple! { $( $rest_name, )* })
}

macro_rules! tuple {
    () => ();
    ( $($name:ident,)+ ) => {
        #[allow(non_snake_case)]
        impl<$($name: Prune),+ > Prune for ($($name,)+) {
            fn prune(&mut self) {
                let ($(ref mut $name,)+) = self;
                $($name.prune();)+
            }
            fn fully_prune(&mut self) {
                let ($(ref mut $name,)+) = self;
                $($name.fully_prune();)+
            }
        }
        peel! { $($name,)+ }
    }
}

tuple! { T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11,}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encodings() {
        /*
        macro_rules! t {
            ($( $value:expr => $expected:expr; )+) => ( $(
                    assert_eq!(($value).encode_to_vec(), &($expected));
            )+)
        }

        t! {
            ((),) => [];
            (1u8,2u8) => [1,2];
            (1u8,(2u8, 3u8)) => [1,2,3];
        }
        */
    }
}
