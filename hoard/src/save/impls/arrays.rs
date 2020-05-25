use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

pub struct ArrayEncoder<T, const N: usize> {
    inner: [T; N],
    idx: usize,
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for ArrayEncoder<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ArrayEncoder")
            .field("inner", &&self.inner[..])
            .field("idx", &self.idx)
            .finish()
    }
}

impl<Q, R, T, const N: usize> Encode<Q, R> for [T; N]
where T: Encode<Q, R>
{
    type EncodePoll = ArrayEncoder<T::EncodePoll, N>;

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
        let mut inner: [MaybeUninit<T::EncodePoll>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut inner[..]);

        for item in self.iter() {
            initializer.push(item.init_encode(dst));
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let inner2 = unsafe { mem::transmute_copy(&inner) };
        assert_eq!(mem::size_of_val(&inner), mem::size_of_val(&inner2));
        assert_eq!(mem::align_of_val(&inner), mem::align_of_val(&inner2));

        ArrayEncoder {
            inner: inner2,
            idx: 0,
        }
    }
}

impl<Q, R, T, const N: usize> SavePoll<Q, R> for ArrayEncoder<T, N>
where T: SavePoll<Q, R>
{
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, mut dst: D) -> Result<D, D::Error> {
        while self.idx < N {
            dst = self.inner[self.idx].save_poll(dst)?;
            self.idx += 1;
        }
        Ok(dst)
    }
}

impl<T, const N: usize> EncodeBlob for ArrayEncoder<T, N>
where T: EncodeBlob
{
    const BLOB_LEN: usize = T::BLOB_LEN * N;

    fn encode_blob<W: WriteBlob>(&self, mut dst: W) -> Result<W::Done, W::Error> {
        assert!(self.idx == N);
        for item in self.inner.iter() {
            dst = dst.write(item)?;
        }
        dst.done()
    }
}
