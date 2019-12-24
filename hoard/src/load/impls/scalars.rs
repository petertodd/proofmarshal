use leint::Le;

use super::*;

macro_rules! impl_decode {
    ($t:ty) => {
        impl Persist for $t {
            type Persist = Self;
        }

        unsafe impl<'a, Z> ValidateChildren<'a, Z> for $t {
            type State = ();
            fn validate_children(_: &Self) -> () {}
            fn poll<V: PtrValidator<Z>>(this: &'a Self, _: &mut (), _: &V) -> Result<&'a Self, V::Error> {
                Ok(this)
            }
        }
        impl<Z> Decode<Z> for $t {
        }
    }
}

macro_rules! impl_all_valid {
    ($( $t:ty, )+) => {$(
        impl ValidateBlob for $t {
            type Error = !;

            fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
                blob.validate_bytes(|blob| unsafe { Ok(blob.assume_valid()) })
            }
        }

        impl_decode!($t);
    )+}
}

impl_all_valid! {
    (),
    u8, i8,
    Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BoolError(());

impl ValidateBlob for bool {
    type Error = BoolError;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        unsafe {
            blob.validate_bytes(|blob|
                match &blob[..] {
                    [0] | [1] => Ok(blob.assume_valid()),
                    [_] => Err(BoolError(())),
                    _ => unreachable!(),
                }
            )
        }
    }
}
impl_decode!(bool);

impl ValidateBlob for ! {
    type Error = !;

    fn validate_blob<B: BlobValidator<Self>>(_: B) -> Result<B::Ok, B::Error> {
        panic!()
    }
}
impl_decode!(!);
