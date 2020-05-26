use super::*;

use std::error::Error;
use std::ops::Deref;
use thiserror::Error;

use hoard::blob::*;
use hoard::load::*;
use hoard::save::*;
use hoard::primitive::Primitive;
use hoard::ptr::AsPtr;

use super::flags::ValidateFlagsBlobError;

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateSumTreeDataBlobError<SumError: Error, PtrError: Error> {
    Flags(<Flags as ValidateBlob>::Error),
    Sum(SumError),
    Ptr(PtrError),
}

impl<T, S, P> ValidateBlob for SumTreeData<T, S, P>
where S: ValidateBlob,
      P: ValidateBlob,
{
    const BLOB_LEN: usize = <u8 as ValidateBlob>::BLOB_LEN +
                            <Digest as ValidateBlob>::BLOB_LEN +
                            <S as ValidateBlob>::BLOB_LEN +
                            <P as ValidateBlob>::BLOB_LEN;

    type Error = ValidateSumTreeDataBlobError<<S as ValidateBlob>::Error, <P as ValidateBlob>::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<Flags>().map_err(ValidateSumTreeDataBlobError::Flags)?;
        blob.field::<Digest>().into_ok();
        blob.field::<S>().map_err(ValidateSumTreeDataBlobError::Sum)?;
        blob.field::<P>().map_err(ValidateSumTreeDataBlobError::Ptr)?;
        unsafe { Ok(blob.finish()) }
    }
}

impl<Z, T, S, P> Decode<Z> for SumTreeData<T, S, P>
where S: Decode<Z>,
      P: Decode<Z>,
{
    fn decode_blob(mut blob: BlobDecoder<Z, Self>) -> Self {
        unsafe {
            Self {
                marker: PhantomData,
                flags: blob.field_unchecked::<u8>().into(),
                tip_digest: blob.field_unchecked::<Digest>().into(),
                sum: blob.field_unchecked::<S>().into(),
                tip: blob.field_unchecked(),
            }
        }
    }
}

unsafe impl<T, S, P> Persist for SumTreeData<T, S, P>
where S: Persist, P: Persist {}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateSumTreeBlobError<SumError: Error, PtrError: Error, ZoneError: Error> {
    Data(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Zone(ZoneError),
    Height(<Height as ValidateBlob>::Error),
}

impl<T, S, P: Ptr, Z> ValidateBlob for SumTree<T, S, P, Z>
where S: ValidateBlob,
      P: ValidateBlob,
      Z: ValidateBlob,
{
    const BLOB_LEN: usize = <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <Z as ValidateBlob>::BLOB_LEN +
                            <Height as ValidateBlob>::BLOB_LEN;

    type Error = ValidateSumTreeBlobError<S::Error, P::Error, Z::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateSumTreeBlobError::Data)?;
        blob.field::<Z>().map_err(ValidateSumTreeBlobError::Zone)?;
        blob.field::<Height>().map_err(ValidateSumTreeBlobError::Height)?;
        unsafe { Ok(blob.finish()) }
    }
}

unsafe impl<T, S, P:Ptr, Z> Persist for SumTree<T, S, P, Z>
where S: Persist, P: Persist, Z: Persist {}

impl<Y, T, S, P: Ptr, Z> Decode<Y> for SumTree<T, S, P, Z>
where S: Decode<Y>,
      P: Decode<Y>,
      Z: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        unsafe {
            Self {
                data: blob.field_unchecked(),
                zone: blob.field_unchecked(),
                height: blob.field_unchecked(),
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub enum ValidateInnerBlobError<SumError: Error, PtrError: Error> {
    Left(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Right(ValidateSumTreeDataBlobError<SumError, PtrError>),
    Height(<Height as ValidateBlob>::Error),
}

impl<T, S, P: Ptr> ValidateBlob for Inner<T, S, P>
where S: ValidateBlob,
      P: ValidateBlob,
{
    const BLOB_LEN: usize = <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <SumTreeData<T, S, P> as ValidateBlob>::BLOB_LEN +
                            <Height as ValidateBlob>::BLOB_LEN;

    type Error = ValidateInnerBlobError<S::Error, P::Error>;

    fn validate_blob<'a>(mut blob: BlobValidator<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateInnerBlobError::Left)?;
        blob.field::<SumTreeData<T, S, P>>().map_err(ValidateInnerBlobError::Right)?;
        blob.field::<Height>().map_err(ValidateInnerBlobError::Height)?;
        unsafe { Ok(blob.finish()) }
    }
}

unsafe impl<T, S, P: Ptr> Persist for Inner<T, S, P>
where S: Persist, P: Persist, {}

impl<Y, T, S, P: Ptr> Decode<Y> for Inner<T, S, P>
where S: Decode<Y>,
      P: Decode<Y>,
{
    fn decode_blob(mut blob: BlobDecoder<Y, Self>) -> Self {
        unsafe {
            Self {
                left: ManuallyDrop::new(blob.field_unchecked()),
                right: ManuallyDrop::new(blob.field_unchecked()),
                height: blob.field_unchecked(),
            }
        }
    }
}

pub struct SumTreeSaver<Q, R, T: Encode<Q, R>, S, P: Ptr, Z: Encode<Q, R> = (), H = ()> {
    tip_digest: Digest,
    sum: S,
    tip: TipState<Q, R, T, S, P>,
    zone: Z::EncodePoll,
    height: Height,
    height_field: H,
}

enum TipState<Q, R, T: Encode<Q, R>, S, P: Ptr> {
    Ready(P::Persist),
    Inner(Box<InnerSaver<Q, R, T, S, P>>),
    Value(T::EncodePoll),
    Done(R),
}

pub struct InnerSaver<Q, R, T: Encode<Q, R>, S, P: Ptr, H = ()> {
    state: InnerSaverState,
    left: SumTreeSaver<Q, R, T, S, P>,
    right: SumTreeSaver<Q, R, T, S, P>,
    height_field: H,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InnerSaverState {
    Ready,
    DoneLeft,
    Done,
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr, Z> Save<Q, R> for SumTreeDyn<T, S, P, Z>
where R: Primitive,
      S: MerkleSum<T> + Primitive,
      Z: Encode<Q, R>,
      P: AsPtr<Q>,
{
    type SavePoll = SumTreeSaver<Q, R, T, S, P, Z>;

    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll {
        SumTreeSaver {
            tip_digest: self.tip_digest(),
            sum: self.sum(),
            zone: self.zone.init_encode(dst),
            height: self.height(),
            height_field: (),
            tip: match self.get_dirty_tip() {
                Err(persist_ptr) => TipState::Ready(persist_ptr),
                Ok(TipRef::Leaf(value)) => TipState::Value(value.init_encode(dst)),
                Ok(TipRef::Inner(inner)) => TipState::Inner(inner.init_save(dst).into()),
            }
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr, Z> Encode<Q, R> for SumTree<T, S, P, Z>
where R: Primitive,
      S: MerkleSum<T> + Primitive,
      Z: Encode<Q, R>,
      P: AsPtr<Q>,
{
    type EncodePoll = SumTreeSaver<Q, R, T, S, P, Z, Height>;

    fn init_encode(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::EncodePoll {
        let saver = self.deref().init_save(dst);

        // Add in the height field
        SumTreeSaver {
            height_field: self.height(),
            tip_digest: saver.tip_digest,
            sum: saver.sum,
            zone: saver.zone,
            height: saver.height,
            tip: saver.tip,
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr, Z: Encode<Q, R>, H> SavePoll<Q, R> for SumTreeSaver<Q, R, T, S, P, Z, H>
where R: Primitive,
      S: MerkleSum<T> + Primitive,
      H: Primitive,
      P: AsPtr<Q>,
{
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, mut dst: D) -> Result<D, D::Error> {
        dst = self.zone.save_poll(dst)?;

        loop {
            self.tip = match &mut self.tip {
                TipState::Ready(persist_ptr) => {
                    let ptr: &P = persist_ptr.as_ptr();
                    let ptr: &Q = ptr.as_ptr();
                    if let Ok(height) = NonZeroHeight::try_from(self.height) {
                        match unsafe { dst.check_dirty::<InnerDyn<T, S, P>>(ptr, height) } {
                            Ok(r_ptr) => TipState::Done(r_ptr),
                            Err(inner) => TipState::Inner(inner.init_save(&dst).into()),
                        }
                    } else {
                        match unsafe { dst.check_dirty::<T>(ptr, ()) } {
                            Ok(r_ptr) => TipState::Done(r_ptr),
                            Err(value) => TipState::Value(value.init_encode(&dst)),
                        }
                    }
                },
                TipState::Inner(inner) => {
                    dst = inner.save_poll(dst)?;
                    let (d, r_ptr) = dst.try_save_ptr(&**inner)?;
                    dst = d;
                    TipState::Done(r_ptr)
                },
                TipState::Value(value) => {
                    dst = value.save_poll(dst)?;
                    let (d, r_ptr) = dst.try_save_ptr(value)?;
                    dst = d;
                    TipState::Done(r_ptr)
                },
                TipState::Done(_) => break Ok(dst),
            }
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr, Z: Encode<Q, R>, H> EncodeBlob for SumTreeSaver<Q, R, T, S, P, Z, H>
where R: Primitive,
      S: Primitive,
      H: Primitive,
{
    const BLOB_LEN: usize = <Flags as ValidateBlob>::BLOB_LEN +
                            <Digest as ValidateBlob>::BLOB_LEN +
                            <S as ValidateBlob>::BLOB_LEN +
                            <R as ValidateBlob>::BLOB_LEN +
                            <H as ValidateBlob>::BLOB_LEN;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        if let TipState::Done(r_ptr) = &self.tip {
            dst.write_primitive(&0u8)?
               .write_primitive(&self.tip_digest)?
               .write_primitive(&self.sum)?
               .write_primitive(r_ptr)?
               .write_primitive(&self.height_field)?
               .done()
        } else {
            panic!()
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr> Save<Q, R> for InnerDyn<T, S, P>
where R: Primitive,
      S: MerkleSum<T> + Primitive,
      P: AsPtr<Q>,
{
    type SavePoll = InnerSaver<Q, R, T, S, P>;

    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll {
        InnerSaver {
            state: InnerSaverState::Ready,
            left: self.left().init_save(dst),
            right: self.right().init_save(dst),
            height_field: (),
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr> SavePoll<Q, R> for InnerSaver<Q, R, T, S, P>
where R: Primitive,
      S: MerkleSum<T> + Primitive,
      P: AsPtr<Q>,
{
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, mut dst: D) -> Result<D, D::Error> {
        loop {
            self.state = match self.state {
                InnerSaverState::Ready => {
                    dst = self.left.save_poll(dst)?;
                    InnerSaverState::DoneLeft
                },
                InnerSaverState::DoneLeft => {
                    dst = self.right.save_poll(dst)?;
                    InnerSaverState::Done
                },
                InnerSaverState::Done => break Ok(dst),
            }
        }
    }
}

impl<Q, R, T: Encode<Q, R>, S, P: Ptr, H> EncodeBlob for InnerSaver<Q, R, T, S, P, H>
where R: Primitive,
      S: Primitive,
      H: Primitive,
{
    const BLOB_LEN: usize = (<SumTreeSaver<Q, R, T, S, P> as EncodeBlob>::BLOB_LEN * 2) + H::BLOB_LEN;
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        assert_eq!(self.state, InnerSaverState::Done);
        dst.write(&self.left)?
           .write(&self.right)?
           .write_primitive(&self.height_field)?
           .done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::prelude::*;

    #[test]
    fn test() {
        let pile = Pile::default();
        let tip = Tree::new_leaf_in(42u8, pile);

        let (buf, offset) = pile.save_to_vec(&tip);
        assert_eq!(offset, 1);
        assert_eq!(buf,
            &[42,
              0, // flags
              0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, // digest
              1,0,0,0,0,0,0,0, // tip ptr
              0, // height
            ][..]
        );

        let tip2 = Tree::new_leaf_in(43u8, pile);
        let tip = tip.try_join_in(tip2, pile).unwrap();

        let (buf, offset) = pile.save_to_vec(&tip);
        assert_eq!(offset, 1 + 1 + (41*2));
        assert_eq!(buf,
            &[42, 43, // leaf values

              // inner
              0, // flags
              0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, // digest
              1,0,0,0,0,0,0,0, // tip ptr, left leaf
              0, // flags
              0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, // digest
              3,0,0,0,0,0,0,0, // tip ptr, right leaf

              // tip
              0, // flags
              0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, // digest
              5,0,0,0,0,0,0,0, // tip ptr, inner
              1, // height
            ][..]
        );
    }
}
