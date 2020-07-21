use std::any::type_name;
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::error::Error;

use thiserror::Error;

use sliceinit::UninitArray;

use super::*;

unsafe impl<T: Persist, const N: usize> Persist for [T; N] {}

#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("array validation failed at index {idx}: {err}")]
pub struct ValidateArrayBlobError<E: Error, const N: usize> {
    idx: usize,
    err: E,
}

impl<E: Error, const N: usize> From<ValidateArrayBlobError<E, N>> for !
where E: Into<!>
{
    fn from(err: ValidateArrayBlobError<E,N>) -> ! {
        err.err.into()
    }
}

unsafe impl<T: ValidateBlob, const N: usize> ValidateBlob for [T; N] {
    type BlobError = ValidateArrayBlobError<T::BlobError, N>;

    #[inline(always)]
    fn try_blob_layout(_: ()) -> Result<BlobLayout, !> {
        Ok(BlobLayout {
            size: T::blob_layout().size() * N,
            niche_start: T::blob_layout().niche_start,
            niche_end: T::blob_layout().niche_end,
            inhabited: T::blob_layout().inhabited,
        })
    }

    fn validate_blob<'a>(blob: Blob<'a, Self>, ignore_padding: bool) -> Result<ValidBlob<'a, Self>, Self::BlobError> {
        let mut fields = blob.validate_fields(ignore_padding);
        for idx in 0 .. N {
            fields.validate_blob::<T>().map_err(|err| ValidateArrayBlobError { idx, err })?;
        }
        unsafe { Ok(fields.finish()) }
    }
}

impl<T: Decode, const N: usize> Load for [T; N] {
    type Ptr = T::Ptr;

    fn decode_blob(blob: ValidBlob<Self>, zone: &<Self::Ptr as Ptr>::BlobZone) -> Self {
        let mut items = blob.decode_fields(zone);
        let mut this = UninitArray::new();
        for _ in 0 .. N {
            let item = unsafe { items.decode_unchecked::<T>() };
            this.push(item);
        }
        items.finish();
        this.done()
    }
}

pub struct ArraySavePoll<Q: Ptr, T: Save<Q>, const N: usize> {
    state: [T::SavePoll; N],
    idx: usize,
}

impl<Q: Ptr, T: Save<Q>, const N: usize> fmt::Debug for ArraySavePoll<Q, T, N>
where T::SavePoll: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("state", &&self.state[..])
            .field("idx", &self.idx)
            .finish()
    }
}

impl<Q: Ptr, T: Saved<Q>, const N: usize> Saved<Q> for [T; N]
where T::Saved: Sized,
{
    type Saved = [T::Saved; N];
}

impl<Q: Ptr, T: Save<Q> + Decode, const N: usize> Save<Q> for [T; N]
where T::Saved: Sized,
{
    type SavePoll = ArraySavePoll<Q, T, N>;

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

impl<Q: Ptr, T: Save<Q> + Decode, const N: usize> EncodeBlob for ArraySavePoll<Q, T, N>
where T::Saved: Sized
{
    type Target = [T::Saved; N];

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}

impl<Q: Ptr, T: Save<Q> + Decode, const N: usize> SavePoll for ArraySavePoll<Q, T, N>
where T::Saved: Sized
{
    type SrcPtr = T::Ptr;

    type DstPtr = Q;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>,
    {
        while self.idx < N {
            self.state[self.idx].save_poll(saver)?;
            self.idx += 1;
        }
        Ok(())
    }
}

/*
/*
impl<Y, Q, T: SavePoll<Y, Q>, const N: usize> SavePoll<Y, Q> for ArraySavePoll<T, N>
where T::Target: BlobSize + Decode<Y>
{
    type Target = [T::Target; N];

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>
    {
        while self.idx < N {
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
    }
}
*/
*/
