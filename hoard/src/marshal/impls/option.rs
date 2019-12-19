use core::any::type_name;
use core::fmt;
use core::mem;

use nonzero::NonZero;

use crate::zone::Ptr;
use crate::marshal::prelude::*;

impl<'a, T, P: Ptr> SaveState<'a, P> for Option<T>
where T: NonZero + SaveState<'a, P>
{
    type State = Option<T::State>;

    fn init_save_state(&'a self) -> Self::State {
        self.as_ref().map(T::init_save_state)
    }
}

unsafe impl<T, P: Ptr> Encode<P> for Option<T>
where T: NonZero + Encode<P>
{
    fn encode_poll<'a, D: Dumper<P>>(&'a self, state: &mut Option<<T as SaveState<'a,P>>::State>, dumper: D)
        -> Result<D, D::Pending>
    {
        match (self, state) {
            (None, None) => Ok(dumper),
            (Some(value), Some(state)) => value.encode_poll(state, dumper),
            _ => unreachable!(),
        }
    }

    fn encode_blob<'a, W: WriteBlob>(&'a self, state: &Option<<T as SaveState<'a,P>>::State>, dst: W) -> Result<W::Ok, W::Error> {
        match (self, state) {
            (None, None) => {
                dst.write_padding(mem::size_of::<Self>())?
                   .finish()
            },
            (Some(value), Some(state)) => {
                dst.write(value, state)?
                   .finish()
            },
            _ => unreachable!()
        }
    }
}

/*
#[derive(Debug)]
pub enum OptionError<E> {
    Discriminant(u8),
    Padding,
    Value(E),
}

fn zeroed(buf: &[u8]) -> bool {
    buf.iter().all(|b| *b == 0)
}

impl<P: Ptr, T: Decode<P>> Decode<P> for Option<T> {
    type Error = OptionError<T::Error>;

    type ValidateChildren = Option<T::ValidateChildren>;

    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        if let Some(niche) = T::BLOB_LAYOUT.niche() {
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

    fn deref_blob<'a>(blob: FullyValidBlob<'a, Self, P>) -> &'a Self
        where Self: Persist
    {
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), Self::BLOB_LAYOUT.size());

        unsafe { blob.assume_valid() }
    }
}
unsafe impl<T: Persist + NonZero> Persist for Option<T> { }

impl<P: Ptr, T: ValidateChildren<P>> ValidateChildren<P> for Option<T> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>
    {
        match self {
            None => Ok(()),
            Some(inner) => inner.validate_children(validator),
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    use crate::pile::{PileMut, Pile};

    #[test]
    fn encodings() {
        let pile = PileMut::default();

        macro_rules! t {
            ($( $value:expr => $expected:expr; )+) => {{
                $(
                    let expected = &$expected;
                    assert_eq!(pile.save_to_vec(&$value), expected);

                    Pile::new(expected, |pile| {
                        let round_trip = pile.load_tip().unwrap();
                        assert_eq!($value, *round_trip);
                    });
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
*/
*/
