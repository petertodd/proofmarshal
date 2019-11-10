use super::*;

use core::mem;

impl<Z> Encode<Z> for ! {
    const ENCODING: Encoding = Encoding::new(0);

    fn encode<E: Encoder<Zone=Z>>(&self, _encoder: E) -> Result<E::Done, E::Error> {
        match *self {}
    }
}

impl<Z> Decode<Z> for ! {
    type Error = !;

    fn decode<D: Decoder<Zone=Z>>(_decoder: D) -> Result<(D::Done, Self), Self::Error> {
        unreachable!()
    }
}

unsafe impl<Z> Persist<Z> for ! {
}

impl<Z> Encode<Z> for () {
    const ENCODING: Encoding = Encoding::new(0);

    fn encode<E: Encoder<Zone=Z>>(&self, encoder: E) -> Result<E::Done, E::Error> {
        todo!()
    }
}

impl<Z> Decode<Z> for () {
    type Error = !;

    fn decode<D: Decoder<Zone=Z>>(_decoder: D) -> Result<(D::Done, Self), Self::Error> {
        todo!()
    }
}

unsafe impl<Z> Persist<Z> for () {
}


impl<Z> Encode<Z> for bool {
    const ENCODING: Encoding = Encoding::new(1);

    fn encode<E: Encoder<Zone=Z>>(&self, encoder: E) -> Result<E::Done, E::Error> {
        encoder.emit_blob(&[*self as u8])
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecodeBoolError(u8);

impl<Z> Decode<Z> for bool {
    type Error = DecodeBoolError;

    fn decode<D: Decoder<Zone=Z>>(_decoder: D) -> Result<(D::Done, Self), Self::Error> {
        todo!()
    }
}

unsafe impl<Z> Persist<Z> for bool {
}

macro_rules! impl_ints {
    ($($t:ty,)+) => {
        $(
            impl<Z> Encode<Z> for $t {
                const ENCODING: Encoding = Encoding::new(mem::size_of::<$t>());

                fn encode<E: Encoder<Zone=Z>>(&self, encoder: E) -> Result<E::Done, E::Error> {
                    encoder.emit_blob(&self.to_le_bytes())
                }
            }

            impl<Z> Decode<Z> for $t {
                type Error = !;

                fn decode<D: Decoder<Zone=Z>>(_decoder: D) -> Result<(D::Done, Self), Self::Error> {
                    todo!()
                }
            }

            unsafe impl<Z> Persist<Z> for $t {
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}
