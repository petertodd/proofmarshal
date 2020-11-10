use std::error::Error;

use thiserror::Error;

use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("FIXME")]
pub enum DecodeTupleBytesError<
    T0: Error,
    T1: Error = !,
    T2: Error = !,
    T3: Error = !,
    T4: Error = !,
    T5: Error = !,
    T6: Error = !,
    T7: Error = !,
    T8: Error = !,
    T9: Error = !,
    T10: Error = !,
    T11: Error = !,
> {
    T0(T0),
    T1(T1),
    T2(T2),
    T3(T3),
    T4(T4),
    T5(T5),
    T6(T6),
    T7(T7),
    T8(T8),
    T9(T9),
    T10(T10),
    T11(T11),
}

/*
macro_rules! peel {
    ($name:ident, $( $rest_name:ident,)* ) => (tuple! { $( $rest_name, )* })
}
*/

macro_rules! impl_tuple {
    ( $($name:ident,)+ ) => {
        #[allow(non_snake_case)]
        impl<$($name: Blob),+ > Blob for ($($name,)+) {
            const SIZE: usize = 0 $(+ <$name as Blob>::SIZE )+;

            type DecodeBytesError = DecodeTupleBytesError<$(<$name as Blob>::DecodeBytesError,)+>;

            fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
                let ($(ref $name,)+) = self;

                let mut dst = dst.write_struct();

                $(
                    dst = dst.write_field($name);
                )+

                dst.done()
            }

            fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
                let mut fields = blob.struct_fields();

                $(
                    let $name = fields.trust_field::<$name>().map_err(DecodeTupleBytesError::$name)?;
                )+

                fields.assert_done();

                Ok(MaybeValid::from(( $($name,)+ )))
            }
        }
    }
}

macro_rules! peel {
    //( $name:ident ) => ();
    ($name:ident, $( $rest_name:ident,)* ) => (tuple! { $( $rest_name, )* })
}

macro_rules! reverse {
    ([] $($reversed:ident)*) => {
        impl_tuple!( $( $reversed, )* );
    };
    ([$first:ident $($rest:ident)*] $($reversed:tt)*) => {
        reverse!([$($rest)*] $first $($reversed)*);  // recursion
    };
}

macro_rules! tuple {
    () => ();
    ( $($name:ident,)+ ) => {
        reverse!([ $( $name )+ ]);
        peel! { $($name,)+ }
    }
}

tuple! ( T11, T10, T9, T8, T7, T6, T5, T4, T3, T2, T1, T0, );

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[test]
    fn encode_bytes() {
        #[track_caller]
        fn t<T: Blob>(value: T, expected: &[u8]) {
            let actual = value.to_blob_bytes();
            assert_eq!(actual, expected);
        }

        t(((),), &[]);
        t((1u8,), &[1]);
        t((0x12u8, 0x3456u16, true),
         &[0x12,
           0x56,
           0x34,
           0x01,
         ]);
    }

    #[test]
    fn decode_bytes() {
        #[track_caller]
        fn t<T: Blob + Eq + fmt::Debug>(buf: &[u8], expected: T) {
            let buf = Bytes::<T>::try_from(buf).unwrap();
            let actual = T::decode_bytes(buf).unwrap().trust();
            assert_eq!(actual, expected);
        }

        t(&[], ());
        t(&[42], (42u8,));
        t(&[42, 0, 0], (42u8, None::<u8>));

        #[track_caller]
        fn e<T: Blob + fmt::Debug>(buf: &[u8], expected: T::DecodeBytesError)
            where T::DecodeBytesError: fmt::Debug + Eq
        {
            let buf = Bytes::<T>::try_from(buf).unwrap();
            let actual = T::decode_bytes(buf).unwrap_err();
            assert_eq!(actual, expected);
        }

        use crate::primitive::impls::DecodeBoolError;

        e::<(bool,)>(&[3], DecodeTupleBytesError::T0(DecodeBoolError));
        e::<(bool,bool)>(&[1,3], DecodeTupleBytesError::T1(DecodeBoolError));
    }
}
