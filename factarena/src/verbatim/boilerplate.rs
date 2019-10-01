use super::{*, layout::*};

impl Verbatim for () {
    type Layout = Primitive<()>;
}

impl Verbatim for bool {
    type Layout = Primitive<bool>;
}


impl<T: Verbatim> Verbatim for Option<T> {
    type Layout = Enum<Variant<T::Layout,
                               Variant<Primitive<()>>>>;
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let l = <<Option<bool> as Verbatim>::Layout as Default>::default();
        dbg!(l.len(&Primitive::<()>::default()));
    }
}

/*
impl<A: ?Sized + VerbatimArena> Verbatim<A> for bool {
    #[inline]
    fn len() -> usize { 1 }

    #[inline]
    fn encode<E: Encoder<A>>(&self, mut encoder: E) -> Result<E::Done, E::Error> {
       encoder.write_bytes([if *self { 1 } else { 0 }])?;
       encoder.done()
    }

    type DecodeError = !;

    #[inline]
    fn decode<D: Decoder<Arena=A>>(mut decoder: D) -> Result<(Self, D::Done), Self::DecodeError> {
        let this = match decoder.read_bytes([0])[0] {
            0 => Ok(false),
            1 => Ok(true),
            x => unimplemented!("{}",x),
        }?;
        Ok((this, decoder.done()))
    }
}

impl<A: ?Sized + VerbatimArena, T: Verbatim<A>> Verbatim<A> for Option<T> {
    #[inline]
    fn len() -> usize {
        if T::is_nonzero() && T::len() > 0 {
            T::len()
        } else {
            1 + T::len()
        }
    }

    #[inline]
    fn encode<E: Encoder<A>>(&self, mut encoder: E) -> Result<E::Done, E::Error> {
        match self {
            Some(v) => {
                if !(T::is_nonzero() && T::len() > 0) {
                    encoder.write(&true)?;
                }
                encoder.write(v)?;
            },
            None => {
                if !(T::is_nonzero() && T::len() > 0) {
                    encoder.write(&false)?;
                }
                encoder.write_zeros(T::len())?;
            }
        };
        encoder.done()
    }

    type DecodeError = !;

    #[inline]
    fn decode<D: Decoder<Arena=A>>(_decoder: D) -> Result<(Self, D::Done), Self::DecodeError> {
        if T::is_nonzero() && T::len() > 0 {
            unimplemented!()
        } else {
            unimplemented!()
        }
    }
}


macro_rules! impl_verbatim_for_ints {
    ( $( $t:ty, )* ) => {
        $(
            impl<A: ?Sized + VerbatimArena> Verbatim<A> for $t {
                #[inline(always)]
                fn len() -> usize {
                    mem::size_of::<Self>()
                }

                #[inline(always)]
                fn encode<E: Encoder<A>>(&self, mut encoder: E) -> Result<E::Done, E::Error>
                {
                    encoder.write_bytes(self.to_le_bytes())?;
                    encoder.done()
                }

                type DecodeError = !;

                #[inline(always)]
                fn decode<D: Decoder<Arena=A>>(mut decoder: D) -> Result<(Self, D::Done), !>
                {
                    let buf = decoder.read_bytes(Default::default());
                    Ok((<$t>::from_le_bytes(buf), decoder.done()))
                }
            }
        )*
    }
}

impl_verbatim_for_ints!{
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}*/
