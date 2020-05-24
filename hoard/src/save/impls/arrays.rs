use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;

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
        where R: ValidateBlob
    {
        assert!(state.idx >= N);
        for (item, state) in self.iter().zip(state.state.iter()) {
            dst = dst.write(item, state)?;
        }
        dst.done()
    }
}
