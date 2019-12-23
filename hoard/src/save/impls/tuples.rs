use super::*;

macro_rules! peel {
    (($name:ident, $state:ident), $( ($rest_name:ident, $rest_state:ident),)* ) => (tuple! { $( ($rest_name,$rest_state), )* })
}

macro_rules! tuple {
    () => ();
    ( $(($name:ident, $state:ident),)+ ) => {
        #[allow(non_snake_case)]
        impl<'a, Z: Zone, $($name: SavePoll<'a, Z>),+ > SavePoll<'a, Z> for ($($name,)+) {
            type State = ( $(<$name as SavePoll<'a, Z>>::State,)+ );

            fn save_children(&'a self) -> Self::State {
                let ($(ref $name,)+) = self;
                ( $($name.save_children(),)+ )
            }

            fn poll<D: Dumper<Z>>(&'a self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
                let ($(ref $name,)+) = self;
                let ($(ref mut $state,)+) = state;
                $(
                    let dumper = $name.poll($state, dumper)?;
                )+
                Ok(dumper)
            }
        }

        #[allow(non_snake_case)]
        impl<Z: Zone, $($name: Encode<Z>),+ > Encode<Z> for ($($name,)+) {
            type Type = ( $( $name::Type, )+ );

            fn encode_blob<'a, W: WriteBlob>(&self, state: &( $(<$name as SavePoll<'a, Z>>::State,)+ ), dst: W)
                -> Result<W::Ok, W::Error>
            {
                assert_eq!(core::mem::align_of::<Self::Type>(), 1);

                let ($(ref $name,)+) = self;
                let ($(ref $state,)+) = state;
                $(
                    let dst = dst.write($name, $state)?;
                )+
                dst.finish()
            }
        }

        peel! { $( ($name, $state), )+ }
    }
}

tuple! { (T0, s0), (T1, s1), (T2, s2), (T3, s3), (T4, s4), (T5, s5), (T6, s6), (T7, s7), (T8, s8), (T9, s9), (T10, s10), (T11, s11),}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encodings() {
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
    }
}
