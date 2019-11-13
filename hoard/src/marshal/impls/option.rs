use super::*;

use core::any::type_name;
use core::fmt;
use core::mem;

use nonzero::NonZero;

impl<P, T: Encode<P>> Encode<P> for Option<T> {
    const BLOB_LAYOUT: BlobLayout = {
        let r = [BlobLayout::new(1).extend(T::BLOB_LAYOUT),
                 T::BLOB_LAYOUT];
        r[T::BLOB_LAYOUT.has_niche() as usize]
    };

    type EncodePoll = Option<T::EncodePoll>;

    fn encode_poll(self) -> Self::EncodePoll {
        self.map(T::encode_poll)
    }
}

impl<P, T: EncodePoll<P>> EncodePoll<P> for Option<T> {
    type Target = Option<T::Target>;

    fn poll<D: Dumper<P>>(&mut self, dumper: D) -> Result<D, D::Pending> {
        match self {
            None => Ok(dumper),
            Some(x) => x.poll(dumper),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        /*
        match self {
            None => {
                if !T::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write_padding(E::Target::BLOB_LAYOUT.size())?
                 .finish()
            },
            Some(v) => {
                if !E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[1])?
                } else {
                    dst
                }.write(v)?
                 .finish()
            },
        }
        */
        todo!()
    }
}

pub enum LoadOptionError<T: Load<P>, P> {
    Discriminant(u8),
    Padding,
    Value(T::Error),
}

impl<T: Load<P>, P> fmt::Debug for LoadOptionError<T, P>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadOptionError::Discriminant(d) => {
                f.debug_tuple("Discriminant")
                 .field(d)
                 .finish()
            },
            LoadOptionError::Padding => {
                f.debug_tuple("Padding")
                 .finish()
            },
            LoadOptionError::Value(e) => {
                f.debug_tuple("Value")
                 .field(e)
                 .finish()
            }
        }
    }
}

fn zeroed(buf: &[u8]) -> bool {
    buf.iter().all(|b| *b == 0)
}

impl<P, T: Decode<P>> Decode<P> for Option<T> {
    type Error = LoadOptionError<T, P>;

    type ValidateChildren = Option<T::ValidateChildren>;

    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        /*
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
        */
        todo!()
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self {
        /*
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
        */
        todo!()
    }

    fn deref_blob<'a>(blob: FullyValidBlob<'a, Self, P>) -> &'a Self
        where Self: Persist
    {
        /*
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), Self::BLOB_LAYOUT.size());

        unsafe { blob.assume_valid() }
        */
        todo!()
    }
}
unsafe impl<T: Persist + NonZero> Persist for Option<T> { }

impl<P, T: ValidateChildren<P>> ValidateChildren<P> for Option<T> {
    /*
    fn validate_children<V>(&mut self, validator: &mut V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<P>
    {
        match self {
            None => Ok(()).into(),
            Some(inner) => inner.validate_children(validator),
        }
    }
    */
}

/*
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
*/
