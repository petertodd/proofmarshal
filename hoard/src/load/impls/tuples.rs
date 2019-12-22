use crate::marshal::prelude::*;
use crate::zone::Ptr;

#[derive(Debug)]
pub struct TupleError;

macro_rules! peel {
    (($name:ident, $state:ident), $( ($rest_name:ident, $rest_state:ident),)* ) => (tuple! { $( ($rest_name,$rest_state), )* })
}

macro_rules! tuple {
    () => ();
    ( $(($name:ident, $state:ident),)+ ) => {
        #[allow(non_snake_case)]
        impl<'a, P: Ptr, $($name: SaveState<'a, P>),+ > SaveState<'a, P> for ($($name,)+) {
            type State = ( $(<$name as SaveState<'a, P>>::State,)+ );

            fn init_save_state(&'a self) -> Self::State {
                assert_eq!(core::mem::align_of::<Self>(), 1);

                let ($(ref $name,)+) = self;
                ( $($name.init_save_state(),)+ )
            }
        }

        #[allow(non_snake_case)]
        unsafe impl<P: Ptr, $($name: Encode<P>),+ > Encode<P> for ($($name,)+) {
            fn encode_poll<'a, D: Dumper<P>>(&'a self, state: &mut ( $(<$name as SaveState<'a, P>>::State,)+ ), dumper: D)
                -> Result<D, D::Pending>
            {
                assert_eq!(core::mem::align_of::<Self>(), 1);
                let ($(ref $name,)+) = self;
                let ($(ref mut $state,)+) = state;
                $(
                    let dumper = $name.encode_poll($state, dumper)?;
                )+
                Ok(dumper)
            }

            fn encode_blob<'a, W: WriteBlob>(&'a self, state: &( $(<$name as SaveState<'a, P>>::State,)+ ), dst: W)
                -> Result<W::Ok, W::Error>
            {
                assert_eq!(core::mem::align_of::<Self>(), 1);
                let ($(ref $name,)+) = self;
                let ($(ref $state,)+) = state;
                $(
                    let dst = dst.write($name, $state)?;
                )+
                dst.finish()
            }
        }

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

        #[allow(non_snake_case)]
        impl<P: Ptr, $($name: ValidateChildren<P>),+ > ValidateChildren<P> for ($($name,)+) {
            fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
                where V: ValidatePtr<P>
            {
                let ($(ref mut $name,)+) = self;
                $(
                    $name.validate_children(validator)?;
                )+
                Ok(())
            }
        }
        */

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
