use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

unsafe impl<T: Persist, const N: usize> Persist for [T; N] {}

impl<T, const N: usize> BlobLen for [T; N]
where T: BlobLen,
{
    const BLOB_LEN: usize = T::BLOB_LEN * N;
}

impl<R, T, const N: usize> Encoded<R> for [T; N]
where T: Encoded<R>,
{
    type Encoded = [T::Encoded; N];
}

pub struct EncodeArrayState<S, const N: usize> {
    state: [S; N],
    idx: usize,
}

impl<S: fmt::Debug, const N: usize> fmt::Debug for EncodeArrayState<S, N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EncodeArrayState")
            .field("state", &&self.state[..])
            .field("idx", &self.idx)
            .finish()
    }
}

impl<'a, Q, R, T, const N: usize> Encode<'a, Q, R> for [T; N]
where T: Encode<'a, Q, R>
{
    type State = EncodeArrayState<T::State, N>;

    fn init_encode_state(&'a self) -> Self::State {
        let mut state: [MaybeUninit<T::State>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut state[..]);

        for item in self.iter() {
            initializer.push(item.init_encode_state());
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let state2 = unsafe { mem::transmute_copy(&state) };
        assert_eq!(mem::size_of_val(&state), mem::size_of_val(&state2));
        assert_eq!(mem::align_of_val(&state), mem::align_of_val(&state2));

        EncodeArrayState {
            state: state2,
            idx: 0,
        }
    }

    fn encode_poll<D>(&'a self, state: &mut Self::State, mut dst: D) -> Result<D, D::Error>
        where D: Dumper<Source=Q, Target=R>,
    {
        while state.idx < N {
            dst = self[state.idx].encode_poll(&mut state.state[state.idx], dst)?;
            state.idx += 1;
        }
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, mut dst: W) -> Result<W::Done, W::Error>
        where R: BlobLen
    {
        assert!(state.idx >= N);
        for (item, state) in self.iter().zip(state.state.iter()) {
            dst = dst.write(item, state)?;
        }
        dst.done()
    }
}

impl<T: Primitive, const N: usize> Primitive for [T; N] {}

/*
#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("array validation failed at index {idx}: {err}")]
pub struct ValidateArrayError<E: Error, const N: usize> {
    idx: usize,
    err: E,
}

impl<E: Error, const N: usize> From<ValidateArrayError<E, N>> for !
where E: Into<!>
{
    fn from(err: ValidateArrayError<E,N>) -> ! {
        err.err.into()
    }
}

impl<T: ValidateBlob, const N: usize> ValidateBlob for [T; N] {
    type Error = ValidateArrayError<T::Error, N>;

    const BLOB_LEN: usize = T::BLOB_LEN * N;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        for idx in 0 .. N {
            blob.field::<T>().map_err(|err| ValidateArrayError { idx, err })?;
        }
        unsafe { Ok(blob.finish()) }
    }
}

impl<Q: Ptr, T, const N: usize> Load<Q> for [T; N]
where T: Decode<Q>
{
    fn decode_blob<'a>(mut blob: BlobLoader<'a, Self, Q>) -> Self {
        let mut r: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for i in 0 .. N {
            let item = unsafe { blob.decode_unchecked() };
            initializer.push(item);
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        blob.finish();
        r2
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;
    use crate::impls::scalars::ValidateBoolError;

    #[test]
    fn validate_blob() {
        let blob = Blob::<[bool; 4]>::try_from(&[0,1,0,1][..]).unwrap();
        let blob = ValidateBlob::validate_blob(blob.into()).unwrap();
        assert_eq!(blob.as_value(), &[false, true, false, true]);

        let blob = Blob::<[bool; 4]>::try_from(&[0,1,0,3][..]).unwrap();
        let err = ValidateBlob::validate_blob(blob.into()).unwrap_err();
        assert_eq!(err.idx, 3);
        assert_eq!(err.err, ValidateBoolError);
    }
}
*/
