use super::*;

macro_rules! peel {
    ($name:ident, $( $rest_name:ident,)* ) => (tuple! { $( $rest_name, )* })
}

macro_rules! tuple {
    () => ();
    ( $($name:ident,)+ ) => {
        #[allow(non_snake_case)]
        impl<Q, R, $($name: Encode<Q, R>),+ > Encode<Q, R> for ($($name,)+) {
            type EncodePoll = ( $(<$name as Encode<Q, R>>::EncodePoll,)+ );

            fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
                let ($(ref $name,)+) = self;
                ( $($name.init_encode(dst),)+ )
            }
        }
        #[allow(non_snake_case)]
        impl<Q, R, $($name: SavePoll<Q, R>),+ > SavePoll<Q, R> for ($($name,)+) {
            fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, dst: D) -> Result<D, D::Error> {
                let ($(ref mut $name,)+) = self;
                $(
                    let dst = $name.save_poll(dst)?;
                )+
                Ok(dst)
            }
        }

        #[allow(non_snake_case)]
        impl<$($name: EncodeBlob),+ > EncodeBlob for ($($name,)+) {
            const BLOB_LEN: usize = ( 0 $( + $name::BLOB_LEN )+ );
            fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                let ($(ref $name,)+) = self;
                $(
                    let dst = dst.write($name)?;
                )+
                dst.done()
            }
        }

        peel! { $( $name, )+ }
    }
}

tuple! { T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, }
