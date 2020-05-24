use std::mem;
use std::num;
use std::slice;

use thiserror::Error;

use leint::Le;

use super::*;

macro_rules! unsafe_impl_all_valid_persist {
    ($($t:ty,)+) => {$(
        impl BlobLen for $t {
            const BLOB_LEN: usize = mem::size_of::<Self>();
        }
        unsafe impl Persist for $t {}

        impl<R> Encoded<R> for $t {
            type Encoded = Self;
        }

        impl<Q, R> Encode<'_, Q, R> for $t {
            type State = ();

            fn init_encode_state(&self) -> Self::State {}

            fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
                where D: Dumper<Source=Q, Target=R>
            {
                Ok(dst)
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

        impl Primitive for $t {}
    )+}
}

unsafe_impl_all_valid_persist! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

macro_rules! impl_nonzero {
    ($($t:ty,)+) => {$(
        impl BlobLen for $t {
            const BLOB_LEN: usize = mem::size_of::<Self>();
        }

        unsafe impl Persist for $t {}

        impl<R> Encoded<R> for $t {
            type Encoded = Self;
        }

        impl<Q, R> Encode<'_, Q, R> for $t {
            type State = ();

            fn init_encode_state(&self) -> Self::State {}

            fn encode_poll<D>(&self, _: &mut (), dst: D) -> Result<D, D::Error>
                where D: Dumper<Source=Q, Target=R>
            {
                Ok(dst)
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

        impl Primitive for $t {}
    )+}
}

impl_nonzero! {
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}

/*
#[non_exhaustive]
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("invalid bool blob")]
pub struct ValidateBoolError;

impl ValidateBlob for bool {
    type Error = ValidateBoolError;
    const BLOB_LEN: usize = mem::size_of::<Self>();

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        match blob.field_bytes(1)[..] {
            [0] | [1] => unsafe { Ok(blob.finish()) },
            _ => Err(ValidateBoolError),
        }
    }
}

macro_rules! unsafe_impl_persist {
    ($($t:ty,)+) => {$(
        unsafe impl Persist for $t {
        }

        impl<Q: Ptr> Load<Q> for $t {
            fn decode_blob<'a>(blob: BlobLoader<'a, Self, Q>) -> Self {
                Self::load_blob(blob).clone()
            }

            fn load_blob<'a>(blob: BlobLoader<'a, Self, Q>) -> Ref<'a, Self> {
                blob.as_value().into()
            }
        }
    )+}
}

unsafe_impl_persist! {
    (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}

/*
impl<Z> Load<Z> for bool {
    fn decode_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Self {
        blob.to_ref().clone()
    }

    fn load_blob<'a>(blob: ValidBlob<'a, Self>, _: &Z) -> Ref<'a, Self> {
        blob.to_ref().into()
    }
}

/*

use super::*;

macro_rules! impl_primitive {
    ($t:ty) => {
        /*
        impl Primitive for $t {
        }
        */
    }
}

macro_rules! impl_save {
    ($t:ty) => {
        impl<R> Saved<R> for $t {
            type Saved = $t;
        }

        impl<Q, R> Save<Q, R> for $t {
            type Thunk = $t;

	    fn save_children<D>(&self, _dst: &mut D) -> Self::Thunk {
		*self
	    }
	}


	impl<Q, R> SavePoll<Q, R> for $t {
            type Target = $t;

	    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
		where D: SavePtr<Source=Q, Target=R>
	    {
		Ok(dst)
	    }

	    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
	    }

	    fn save_blob<W: SaveBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                let dst = dst.alloc(mem::size_of::<$t>())?;
                //<Self as SavePoll<$t, Q,R>>::encode_blob(self, dst)
                todo!()
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

impl<Q, R> Save<Q, R> for bool {
    type Thunk = bool;

    fn save_children<D>(&self, _dst: &mut D) -> Self::Thunk {
        *self
    }
}


impl<Q, R> SavePoll<Q, R> for bool {
    type Target = bool;

    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>
    {
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        let src = unsafe { slice::from_raw_parts(
            self as *const _ as *const u8,
            mem::size_of::<bool>()
        )};

        dst.write_bytes(src)?
           .done()
    }

    fn save_blob<W: SaveBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<bool>())?;
        //<Self as SavePoll<bool, Q,R>>::encode_blob(self, dst)
        todo!()
    }
}

impl_primitive!(bool);

/*
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
*/
*/
*/
*/
