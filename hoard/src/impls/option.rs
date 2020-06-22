use super::*;

use thiserror::Error;

impl<T: BlobSize> BlobSize for Option<T> {
    const BLOB_LAYOUT: BlobLayout = {
        if T::BLOB_LAYOUT.has_niche() {
            T::BLOB_LAYOUT
        } else {
            BlobLayout::new(1).extend(T::BLOB_LAYOUT)
        }
    };
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("FIXME")]
pub enum ValidateOptionBlobError<ValueError: std::error::Error> {
    Discriminant,
    Padding,
    Value(ValueError),
}

impl<V: Copy, T: ValidateBlob<V>> ValidateBlob<V> for Option<T> {
    type Error = ValidateOptionBlobError<T::Error>;

    fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
        if let Some(niche) = T::BLOB_LAYOUT.niche() {
            let (lhs_padding, rest) = blob.as_bytes().split_at(niche.start);
            let (niche_bytes, rhs_padding) = rest.split_at(niche.end);

            if niche_bytes.iter().all(|b| *b == 0) {
                /*
                padval.validate_padding(lhs_padding).map_err(ValidateOptionBlobError::Padding)?;
                padval.validate_padding(rhs_padding).map_err(ValidateOptionBlobError::Padding)?;

                unsafe { Ok(blob.assume_valid()) }
                */ todo!()
            } else {
                let mut fields = blob.validate_fields(padval);

                fields.validate_blob::<T>().map_err(ValidateOptionBlobError::Value)?;
                unsafe { Ok(fields.finish()) }
            }
        } else {
            let mut fields = blob.validate_fields(padval);

            match fields.validate_blob::<u8>().into_ok().as_value() {
                1 => {
                    fields.validate_blob::<T>().map_err(ValidateOptionBlobError::Value)?;
                    unsafe { Ok(fields.finish()) }
                },
                0 => {
                    /*
                    let padding = fields.field_bytes(T::blob_layout().size());
                    padval.validate_padding(padding).map_err(ValidateOptionBlobError::Padding)?;
                    unsafe { Ok(fields.finish()) }
                    */ todo!()
                },
                x => Err(ValidateOptionBlobError::Discriminant),
            }
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    type Zone = T::Zone;
    type Ptr = T::Ptr;

    fn decode_blob(blob: ValidBlob<Self>, zone: &Self::Zone) -> Self {
        todo!()
    }
}

/*
#[derive(Debug)]
pub struct OptionSavePoll<T>(Option<T>);

impl<Y: Zone, T: SaveIn<Y>> SaveIn<Y> for Option<T>
where T::Saved: Sized,
{
    type Saved = Option<T::Saved>;
    type SavePoll = OptionSavePoll<T::SavePoll>;

    fn init_save(&self) -> Self::SavePoll {
        OptionSavePoll(self.as_ref().map(T::init_save))
    }
}

impl<Y: Zone, T: SavePoll<Y>> SavePoll<Y> for OptionSavePoll<T>
where T::Target: Sized,
{
    type Target = Option<T::Target>;

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<<Self::Target as Load>::Ptr, DstZone = Y>,
    {
        match self.0 {
            Some(ref mut inner) => inner.save_children(dst),
            None => Ok(()),
        }
    }

    fn save_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}
*/

/*
impl<Z, T: Decode<Z>> Decode<Z> for Option<T> {}

impl<Y, T: Saved<Y>> Saved<Y> for Option<T>
where T::Saved: Decode<Y>,
{
    type Saved = Option<T::Saved>;
}

#[derive(Debug)]
pub struct OptionSavePoll<T>(Option<T>);

impl<Y, Q, T: SavePoll<Y, Q>> SavePoll<Y, Q> for OptionSavePoll<T>
where T::Target: BlobSize + Decode<Y>
{
    type Target = Option<T::Target>;

    fn target_metadata(&self) -> <Self::Target as Pointee>::Metadata {
    }

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>
    {
        match &mut self.0 {
            Some(value) => value.save_children(dst),
            None => Ok(()),
        }
    }

    fn save_blob<W: WriteBlob<Y, Q>>(&self, mut dst: W) -> Result<W::Done, W::Error>
        where Y: BlobZone
    {
        if let Some(niche) = T::Target::blob_layout().niche() {
            match &self.0 {
                None => {
                    for _ in 0 .. T::Target::blob_layout().size() {
                        dst = dst.write_bytes(&[0])?;
                    }
                    dst.done()
                }
                Some(value) => dst.write(value)?
                                  .done()
            }
        } else {
            match &self.0 {
                None => {
                    dst.write_bytes(&[0])?
                       .write_padding(T::Target::blob_layout().size())?
                       .done()
                },
                Some(value) => {
                    dst.write_bytes(&[1])?
                       .write(value)?
                       .done()
                }
            }
        }
    }
}

impl<Y, Q, T: Save<Y, Q>> Save<Y, Q> for Option<T>
where T::Saved: BlobSize + Decode<Y>,
{
    type SavePoll = OptionSavePoll<T::SavePoll>;

    fn init_save(&self) -> Self::SavePoll {
        OptionSavePoll(
            self.as_ref().map(|value| value.init_save())
        )
    }
}

/*
impl<Y, P, T: EncodeBlobPoll<Y, P>> EncodeBlobPoll<Y, P> for Option<T> {
    type Target = Option<T::Target>;

    fn encode_blob_poll<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>
    {
        match self {
            Some(inner) => inner.encode_blob_poll(dst),
            None => Ok(()),
        }
    }

    fn encode_blob<W: Write>(&self, dst: &mut W) -> Result<(), W::Error> {
        if let Some(niche) = T::Target::blob_layout().niche() {
            match self {
                None => {
                    for _ in 0 .. T::Target::blob_layout().size() {
                        dst.write(&[0])?;
                    };
                    Ok(())
                },
                Some(inner) => inner.encode_blob(dst),
            }
        } else {
            match self {
                None => {
                    dst.write(&[0])?;
                    dst.write_padding(T::Target::blob_layout().size())?;
                    Ok(())
                },
                Some(inner) => {
                    dst.write(&[1])?;
                    inner.encode_blob(dst)
                },
            }
        }
    }
}

impl<Y, P, T: EncodeBlob<Y, P>> EncodeBlob<Y, P> for Option<T> {
    type Encoded = Option<T::Encoded>;
    type EncodeBlobPoll = Option<T::EncodeBlobPoll>;

    fn init_encode_blob<D>(&self, dst: &D) -> Result<Self::EncodeBlobPoll, D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>
    {
        match self {
            Some(inner) => Ok(Some(inner.init_encode_blob(dst)?)),
            None => Ok(None),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;

    use crate::blob::padding::*;

    #[test]
    fn no_niche() {
        let blob = Blob::<Option<()>>::try_from(&[0][..]).unwrap();
        let _valid_blob = ValidateBlob::validate_blob(blob, CheckPadding).unwrap();

        let blob = Blob::<Option<()>>::try_from(&[1][..]).unwrap();
        let _valid_blob = ValidateBlob::validate_blob(blob, CheckPadding).unwrap();

        let blob = Blob::<Option<()>>::try_from(&[2][..]).unwrap();
        let err = ValidateBlob::validate_blob(blob, CheckPadding).unwrap_err();
        assert_eq!(err, ValidateOptionBlobError::Discriminant);
    }
}
*/
*/
