use std::any::type_name;
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::UninitArray;

use super::*;

unsafe impl<T: Persist, const N: usize> Persist for [T; N] {}

impl<T: BlobSize, const N: usize> BlobSize for [T; N] {
    const BLOB_LAYOUT: BlobLayout = BlobLayout {
        size: T::BLOB_LAYOUT.size() * N,
        niche_start: T::BLOB_LAYOUT.niche_start,
        niche_end: T::BLOB_LAYOUT.niche_end,
        inhabited: T::BLOB_LAYOUT.inhabited,
    };
}

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

impl<V: Copy, T: BlobSize + ValidateBlob<V>, const N: usize> ValidateBlob<V> for [T; N] {
    type Error = ValidateArrayError<T::Error, N>;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut fields = blob.validate_fields(padval);
        for idx in 0 .. N {
            fields.field::<T>().map_err(|err| ValidateArrayError { idx, err })?;
        }
        unsafe { Ok(fields.finish()) }
    }
}

impl<Z, T: Decode<Z>, const N: usize> Load<Z> for [T; N] {
    fn decode_blob(blob: ValidBlob<Self>, zone: &Z) -> Self
        where Z: BlobZone
    {
        todo!()
    }
}

impl<Z, T: Decode<Z>, const N: usize> Decode<Z> for [T; N] {}

impl<Y, T: Saved<Y>, const N: usize> Saved<Y> for [T; N]
where T::Saved: Decode<Y>
{
    type Saved = [T::Saved; N];
}

pub struct ArraySavePoll<T, const N: usize> {
    state: [T; N],
    idx: usize,
}

impl<T, const N: usize> fmt::Debug for ArraySavePoll<T, N>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("state", &&self.state[..])
            .field("idx", &self.idx)
            .finish()
    }
}

impl<Y, Q, T: SavePoll<Y, Q>, const N: usize> SavePoll<Y, Q> for ArraySavePoll<T, N>
where T::Target: BlobSize + Decode<Y>
{
    type Target = [T::Target; N];

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>
    {
        while self.idx < N {
            self.state[self.idx].save_children(dst)?;
            self.idx += 1;
        }
        Ok(())
    }

    fn save_blob<W: WriteBlob<Y, Q>>(&self, mut dst: W) -> Result<W::Done, W::Error> {
        assert_eq!(self.idx, N, "polling incomplete");

        for item in self.state.iter() {
            dst = dst.write(item)?;
        }
        dst.done()
    }
}

impl<Y, Q, T: Save<Y, Q>, const N: usize> Save<Y, Q> for [T; N]
where T::Saved: BlobSize + Decode<Y>,
{
    type SavePoll = ArraySavePoll<T::SavePoll, N>;

    fn init_save(&self) -> Self::SavePoll {
        let mut state = UninitArray::new();
        for item in self.iter() {
            state.push(item.init_save());
        }
        ArraySavePoll {
            state: state.done(),
            idx: 0,
        }
    }
}
