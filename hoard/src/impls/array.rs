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

impl<T: BlobSize, const N: usize> BlobSize for [T; N] {
    const BLOB_LAYOUT: BlobLayout = BlobLayout {
        size: T::BLOB_LAYOUT.size() * N,
        niche_start: T::BLOB_LAYOUT.niche_start,
        niche_end: T::BLOB_LAYOUT.niche_end,
        inhabited: T::BLOB_LAYOUT.inhabited,
    };
}

impl<V: Copy, T: ValidateBlob<V>, const N: usize> ValidateBlob<V> for [T; N] {
    type Error = ValidateArrayBlobError<T::Error, N>;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        let mut fields = blob.validate_fields(padval);
        for idx in 0 .. N {
            fields.validate_blob::<T>().map_err(|err| ValidateArrayBlobError { idx, err })?;
        }
        unsafe { Ok(fields.finish()) }
    }
}

impl<T: Decode, const N: usize> Decode for [T; N] {
    type Zone = T::Zone;
    type Ptr = T::Ptr;

    fn decode_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self {
        todo!()
    }
}


/*
impl<Y: Zone, T: Encode<Y>, const N: usize> Encode<Y> for [T; N] {
    type Encoded = [T::Encoded; N];
    type EncodePoll = ArrayEncodePoll<Y, T, N>;

    fn init_encode(&self) -> Self::EncodePoll {
        let mut state = UninitArray::new();
        for item in self.iter() {
            state.push(item.init_encode());
        }
        ArrayEncodePoll {
            state: state.done(),
            idx: 0,
        }
    }
}

pub struct ArrayEncodePoll<Y: Zone, T: Encode<Y>, const N: usize> {
    state: [T::EncodePoll; N],
    idx: usize,
}

impl<Y: Zone, T: Encode<Y>, const N: usize> fmt::Debug for ArrayEncodePoll<Y, T, N>
where T::EncodePoll: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("state", &&self.state[..])
            .field("idx", &self.idx)
            .finish()
    }
}

impl<Y: Zone, T: Encode<Y>, const N: usize> EncodePoll for ArrayEncodePoll<Y, T, N> {
    type DstZone = Y;
    type Target = [T::Encoded; N];
    type SrcPtr = <T::Ptr as Ptr>::Persist;

    fn encode_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<DstZone = Self::DstZone>,
    {
        while self.idx < N {
            self.state[self.idx].encode_poll(saver)?;
            self.idx += 1;
        }
        Ok(())
    }
}

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
