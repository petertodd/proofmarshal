use leint::Le;

use super::*;

macro_rules! impl_all_valid {
    ($( $t:ty, )+) => {$(
        impl Validate for $t {
            type Error = !;

            fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
                blob.validate_bytes(|blob| unsafe { Ok(blob.assume_valid()) })
            }
        }

        unsafe impl<Z: Zone> Load<Z> for $t {

            type ValidateChildren = ();
            fn validate_children(&self) -> () {}
        }
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

impl ValidationError for BoolError {
}

impl Validate for bool {
    type Error = BoolError;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
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
unsafe impl<Z: Zone> Load<Z> for bool {
    type ValidateChildren = ();
    fn validate_children(&self) -> () {}
}

impl Validate for ! {
    type Error = !;

    fn validate<B: BlobValidator<!>>(blob: B) -> Result<B::Ok, B::Error> {
        panic!()
    }
}
unsafe impl<Z: Zone> Load<Z> for ! {
    type ValidateChildren = ();
    fn validate_children(&self) -> () {
        match *self {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
