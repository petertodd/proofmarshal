use super::*;

use core::any::type_name;
use core::fmt;
use core::mem::{self, MaybeUninit};

use sliceinit::SliceInitializer;

/// Returned when decoding an array fails.
pub struct DecodeArrayError<T: Verbatim<P>, P> {
    idx: usize,
    err: T::Error,
}

impl<T: Verbatim<P>, P> fmt::Debug for DecodeArrayError<T,P>
where T::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("idx", &self.idx)
            .field("err", &self.err)
            .finish()
    }
}

macro_rules! array_impls {
    ($($N:literal)+) => {
        $(
            impl<T: Verbatim<P>, P> Verbatim<P> for [T;$N] {
                const LEN: usize = $N * T::LEN;
                const NONZERO_NICHE: bool = T::NONZERO_NICHE;

                type Error = DecodeArrayError<T, P>;

                #[inline]
                fn decode(src: &[u8], ptr_decoder: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
                    assert_eq!(src.len(), Self::LEN);

                    let mut uninit: [MaybeUninit<T>; $N] = unsafe { MaybeUninit::uninit().assume_init() };
                    let mut initializer = SliceInitializer::new(&mut uninit);

                    for (idx, chunk) in src.chunks(T::LEN).enumerate() {
                        let item = T::decode(chunk, ptr_decoder)
                                     .map_err(|err| DecodeArrayError { idx, err })?;

                        initializer.push(item);
                    }
                    initializer.done();

                    unsafe { mem::transmute_copy(&uninit) }
                }

                #[inline]
                fn encode<W: io::Write>(&self, mut dst: W, ptr_encoder: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
                    for field in self {
                        dst = field.encode(dst, ptr_encoder)?;
                    }
                    Ok(dst)
                }
            }
        )+
    }
}

array_impls! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

#[cfg(test)]
mod tests {
}
