use crate::pointee::Pointee;
use crate::marshal::blob::WriteBlob;

use super::Dumper;

pub trait Encoded<Z> : Sized + Pointee<Metadata=()> {
    type Encoded : Sized + Pointee<Metadata=()>;
}

pub trait Encode<'a, Z> : Encoded<Z> {
    type State;

    fn make_encode_state(&'a self) -> Self::State;

    fn encode_poll<D>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error>
        where D: Dumper<Z>;

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;
}

pub trait EncodePrimitive : for<'a> Encode<'a, !, State=(), Encoded=Self> {
    #[inline(always)]
    fn encode_primitive_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(&(), dst)
    }
}

#[macro_export]
macro_rules! impl_encode_for_primitive {
    ($t:ty, |$this:ident, $dst:ident| $encode_body:tt) => {
        impl<Z> $crate::marshal::encode::Encoded<Z> for $t {
            type Encoded = $t;
        }

        impl<Z> $crate::marshal::encode::Encode<'_, Z> for $t {
            type State = ();

            #[inline(always)]
            fn make_encode_state(&self) -> () {}

            #[inline(always)]
            fn encode_poll<D: $crate::marshal::Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
                Ok(dumper)
            }

            #[inline(always)]
            fn encode_blob<W: $crate::marshal::blob::WriteBlob>(&self, _: &(), $dst: W) -> Result<W::Ok, W::Error> {
                let $this = self;
                $encode_body
            }
        }

        impl $crate::marshal::encode::EncodePrimitive for $t {}
    }
}
