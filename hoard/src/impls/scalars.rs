use std::mem;
use std::num;
use std::slice;

use thiserror::Error;

use leint::Le;

use super::*;

macro_rules! impl_primitive {
    ($t:ty) => {
        impl Primitive for $t {
        }
    }
}

macro_rules! impl_save {
    ($t:ty) => {
        impl<R> Saved<R> for $t {
            type Saved = $t;
        }

        impl<Q, R> Save<'_, Q, R> for $t {
            type State = ();

            fn init_save_state(&self) -> () {}

            fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut (), dst: D) -> Result<D, D::Error> {
                Ok(dst)
            }

            fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
                let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
                <Self as Save<Q,R>>::encode_blob(self, state, dst)
            }

            fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
                let src = unsafe { slice::from_raw_parts(
                    self as *const _ as *const u8,
                    mem::size_of::<$t>()
                )};

                dst.write_bytes(src)?
                   .done()
            }
        }
    }
}

macro_rules! impl_all_valid {
    ($($t:ty,)+) => {$(
        impl Load for $t {
            type Error = !;

            fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                let blob = Blob::from(blob);
                unsafe { Ok(blob.assume_valid()) }
            }
        }

        impl_save!($t);
        impl_primitive!($t);
    )+}
}

impl_all_valid! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

#[non_exhaustive]
#[derive(Error, Debug)]
#[error("invalid bool blob")]
pub struct ValidateBoolError;

impl Load for bool {
    type Error = ValidateBoolError;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let blob = Blob::from(blob);
        match blob[0] {
            0 | 1 => unsafe { Ok(blob.assume_valid()) },
            _ => Err(ValidateBoolError),
        }
    }
}

impl<R> Saved<R> for bool {
    type Saved = Self;
}

impl<Q, R> Save<'_, Q, R> for bool {
    type State = ();

    fn init_save_state(&self) -> () {}
    fn save_poll<D: SavePtr<Q, R>>(&self, _: &mut (), dst: D) -> Result<D, D::Error> {
        Ok(dst)
    }

    fn save_blob<W: SaveBlob>(&self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        <Self as Save<Q,R>>::encode_blob(self, state, dst)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}

impl_primitive!(bool);

#[non_exhaustive]
#[derive(Debug, Error)]
#[error("non-zero int")]
pub struct LoadNonZeroIntError;

macro_rules! impl_nonzero {
    ($($t:ty,)+) => {$(
        impl Load for $t {
            type Error = !;

            fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
                /*
                blob.validate_bytes(|blob| {
                    if blob.iter().all(|b| *b == 0) {
                        Err(ValidateNonZeroIntError)
                    } else {
                        Ok(unsafe { blob.assume_valid() })
                    }
                })*/ todo!()
            }
        }

        impl_save!($t);
        impl_primitive!($t);
    )+}
}

impl_nonzero! {
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}
