use super::*;

use core::any::type_name;
use core::fmt;
use core::mem;

use nonzero::NonZero;

const fn option_blob_layout(inner: BlobLayout) -> BlobLayout {
    let r = [BlobLayout::new(1).extend(inner),
             inner];
    r[inner.has_niche() as usize]
}

unsafe impl<Z, T: Encode<Z>> Encode<Z> for Option<T> {
    fn blob_layout() -> BlobLayout
        where Z: BlobZone
    {
        let inner = T::blob_layout();

        BlobLayout::new(
            if inner.has_niche() {
                inner.size()
            } else {
                1 + inner.size()
            }
        )
    }

    type State = Option<T::State>;

    fn init_encode_state(&self) -> Self::State {
        self.as_ref().map(T::init_encode_state)
    }

    fn encode_poll<D: SavePtr<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>
        where Z: BlobZone
    {
        match (self, state) {
            (None, None) => Ok(dumper),
            (Some(value), Some(state)) => value.encode_poll(state, dumper),
            _ => unreachable!(),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>
        where Z: BlobZone
    {
        match (self, state) {
            (None, None) => {
                if !T::blob_layout().has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write_padding(T::blob_layout().size())?
                 .finish()
            },
            (Some(value), Some(state)) => {
                if !T::blob_layout().has_niche() {
                    dst.write_bytes(&[1])?
                } else {
                    dst
                }.write(value, state)?
                 .finish()
            },
            _ => unreachable!()
        }
    }
}

#[derive(Debug)]
pub enum OptionError<E> {
    Discriminant(u8),
    Padding,
    Value(E),
}

fn zeroed(buf: &[u8]) -> bool {
    buf.iter().all(|b| *b == 0)
}

impl<Z, T: Decode<Z>> Decode<Z> for Option<T> {
    type Error = OptionError<T::Error>;

    type ValidateChildren = Option<T::ValidateChildren>;

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>
        where Z: BlobZone
    {
        if let Some(niche) = T::blob_layout().niche() {
            let (left_padding, _) = blob.split_at(niche.start);
            let (_, right_padding) = blob.split_at(niche.end);
            let niche = &blob[niche];
            assert!(niche.len() > 0);

            if zeroed(niche) {
                if zeroed(left_padding) && zeroed(right_padding) {
                    Ok(blob.assume_valid(None))
                } else {
                    Err(OptionError::Padding)
                }
            } else {
                let mut v = blob.validate_struct();
                let state = v.field::<T>().map_err(|e| OptionError::Value(e))?;
                Ok(v.done(Some(state)))
            }
        } else {
            match blob.validate_enum() {
                (0, v) => v.done(None).ok().ok_or(OptionError::Padding),
                (1, mut v) => {
                    let state = v.field::<T>().map_err(|e| OptionError::Value(e))?;
                    Ok(v.done(Some(state)).unwrap())
                },
                (x, _) => Err(OptionError::Discriminant(x)),
            }
        }
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Self
        where Z: BlobZone
    {
        if let Some(niche) = T::blob_layout().niche() {
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

    fn deref_blob<'a>(blob: FullyValidBlob<'a, Self, Z>) -> &'a Self
        where Self: Persist, Z: BlobZone
    {
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), Self::blob_layout().size());

        unsafe { blob.assume_valid() }
    }
}
unsafe impl<T: Persist + NonZero> Persist for Option<T> { }

impl<Z, T: ValidateChildren<Z>> ValidateChildren<Z> for Option<T> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>, Z: BlobZone
    {
        match self {
            None => Ok(()),
            Some(inner) => inner.validate_children(validator),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::convert::TryFrom;

    #[test]
    fn encodings() {
        macro_rules! t {
            ($( $value:expr => $expected:expr; )+) => {{
                $(
                    let expected = &$expected;
                    assert_eq!(encode(&$value), expected);
                    let round_trip = decode(expected).unwrap();
                    assert_eq!($value, round_trip);
                )+
            }}
        }

        t! {
            None::<()> => [0];
            Some(()) => [1];

            None::<u8> => [0,0];
            Some(24u8) => [1,24];

            None::<Option<()>> => [0,0];
            Some(None::<()>)   => [1,0];
            Some(Some(()))     => [1,1];
        }
    }
}
