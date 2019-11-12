use super::*;

use core::any::type_name;
use core::fmt;

impl<Z: Zone, T: Save<Z>> Save<Z> for Option<T> {
    const BLOB_LAYOUT: BlobLayout = {
        let r = [BlobLayout::new(1).extend(T::BLOB_LAYOUT),
                 T::BLOB_LAYOUT];
        r[T::BLOB_LAYOUT.has_niche() as usize]
    };

    type SavePoll = Option<T::SavePoll>;

    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        this.take_sized()
            .map(|v| T::save_poll(v))
    }
}

impl<Z: Zone, E: SavePoll<Z>> SavePoll<Z> for Option<E>
where E::Target: Sized,
{
    type Target = Option<E::Target>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Z>
    {
        match self {
            None => Ok(()).into(),
            Some(e) => e.save_children(ptr_saver),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        match self {
            None => {
                if !E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write_padding(E::Target::BLOB_LAYOUT.size())?
                 .done()
            },
            Some(v) => {
                if !E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[1])?
                } else {
                    dst
                }.write(v)?
                 .done()
            },
        }
    }
}

pub enum VerifyOptionError<T: Load<Z>, Z: Zone> {
    Discriminant(u8),
    Padding,
    Value(T::Error),
}

impl<T: Load<Z>, Z: Zone> fmt::Debug for VerifyOptionError<T, Z>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VerifyOptionError::Discriminant(d) => f.debug_tuple("Discriminant")
                                                   .field(d)
                                                   .finish(),
            VerifyOptionError::Padding => f.debug_tuple("Padding")
                                           .finish(),
            VerifyOptionError::Value(e) => f.debug_tuple("Value")
                                            .field(e)
                                            .finish(),
        }
    }
}


fn zeroed(buf: &[u8]) -> bool {
    buf.iter().all(|b| *b == 0)
}

impl<Z: Zone, T: Load<Z>> Load<Z> for Option<T> {
    type Error = VerifyOptionError<T, Z>;

    type ValidateChildren = Option<T::ValidateChildren>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        if let Some(niche) = T::BLOB_LAYOUT.niche() {
            let (left_padding, _) = blob.split_at(niche.start);
            let (_, right_padding) = blob.split_at(niche.end);
            let niche = &blob[niche];
            assert!(niche.len() > 0);

            if zeroed(niche) {
                if zeroed(left_padding) && zeroed(right_padding) {
                    Ok(blob.assume_valid(None))
                } else {
                    Err(VerifyOptionError::Padding)
                }
            } else {
                let mut v = blob.validate();
                let state = v.field::<T>().map_err(|e| VerifyOptionError::Value(e))?;
                Ok(v.done(Some(state)))
            }
        } else {
            match blob.validate_enum() {
                (0, v) => v.done(None).ok().ok_or(VerifyOptionError::Padding),
                (1, mut v) => {
                    let state = v.field::<T>().map_err(|e| VerifyOptionError::Value(e))?;
                    Ok(v.done(Some(state)).unwrap())
                },
                (x, _) => Err(VerifyOptionError::Discriminant(x)),
            }
        }
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self {
        if let Some(niche) = T::BLOB_LAYOUT.niche() {
            let niche = &blob[niche];

            if zeroed(niche) {
                None
            } else {
                let mut decoder = blob.decode_struct(loader);
                Some(decoder.field::<T>())
            }
        } else {
            match blob.decode_enum(loader) {
                (0, _) => None,
                (1, mut decoder) => Some(decoder.field::<T>()),
                (x, _) => unreachable!("invalid {} discriminant {}", type_name::<Self>(), x)
            }
        }
    }
}

impl<Z: Zone, T: ValidateChildren<Z>> ValidateChildren<Z> for Option<T> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        match self {
            None => Ok(()).into(),
            Some(inner) => inner.validate_children(validator),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;

    #[test]
    fn test() {
        assert_eq!(encode(Some(123u8)),
                   &[1,123]);

        assert_eq!(encode(None::<u8>),
                   &[0,0]);

        let blob = Blob::<Option<u8>,!>::try_from(&[1,123][..]).unwrap();
        assert_eq!(try_decode(blob).unwrap(),
                   Ref::Owned(Some(123)));

        let blob = Blob::<Option<u8>,!>::try_from(&[0,0][..]).unwrap();
        assert_eq!(try_decode(blob).unwrap(),
                   Ref::Owned(None));
    }
}
