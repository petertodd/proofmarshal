use super::*;

#[derive(Debug)]
pub struct TupleError;

macro_rules! peel {
    (($name:ident, $state:ident), $( ($rest_name:ident, $rest_state:ident),)* ) => (tuple! { $( ($rest_name,$rest_state), )* })
}

macro_rules! tuple {
    () => ();
    ( $(($name:ident, $state:ident),)+ ) => {
        /*
        impl<P: Ptr, $($name: Decode<P>),+ > Decode<P> for ($($name,)+) {
            type Error = TupleError;

            type ValidateChildren = ( $( <$name>::ValidateChildren, )+ );
            fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
                let mut fields = blob.validate_struct();
                let state = (
                    $( fields.field::<$name>().map_err(|_| TupleError)?, )+
                );
                Ok(fields.done(state))
            }

            fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self {
                let mut fields = blob.decode_struct(loader);
                ( $( fields.field::<$name>(), )+ )
            }
        }
        */

        #[allow(non_snake_case)]
        impl<Z: Zone, $($name: ValidateChildren<Z>),+ > ValidateChildren<Z> for ($($name,)+) {
            fn poll<V>(&mut self, validator: &V) -> Result<(), V::Error>
                where V: PtrValidator<Z>
            {
                let ($(ref mut $name,)+) = self;
                $(
                    $name.poll(validator)?;
                )+
                Ok(())
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
        /*
        let pile = PileMut::default();

        macro_rules! t {
            ($( $value:expr => $expected:expr; )+) => ( $(
                    assert_eq!(pile.save_to_vec(&$value), &$expected);
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
