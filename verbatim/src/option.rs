//! `Option<T>` with `T: NonZero + Persist`
//!
//! We can persist these types because of Rust's non-zero optimization.

use super::*;

use core::fmt;

impl<P, T> Verbatim<P> for Option<T>
where T: Verbatim<P>
{
    type Error = OptionDecodeError<T, P>;

    const LEN: usize = (T::NONZERO_NICHE as usize * T::LEN) +
                       (!T::NONZERO_NICHE as usize * (1 + T::LEN));
    const NONZERO_NICHE: bool = false;

    #[inline(always)]
    fn encode<W: io::Write>(&self, mut dst: W, ptr_encoder: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        if T::NONZERO_NICHE {
            assert!(T::LEN > 0);
        } else {
            dst.write_all(&[self.is_some() as u8])?;
        }
        match self {
            None => {
                for _ in 0 .. T::LEN {
                    dst.write(&[0])?;
                }
                Ok(dst)
            },
            Some(v) => v.encode(dst, ptr_encoder),
        }
    }

    #[inline(always)]
    fn decode(src: &[u8], ptr_decoder: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        assert_eq!(src.len(), Self::LEN);

        Ok(
            if T::NONZERO_NICHE {
                assert!(T::LEN > 0);

                if src.iter().all(|b| *b == 0) {
                    None
                } else {
                    Some(T::decode(src, ptr_decoder)
                           .map_err(|err| OptionDecodeError::Some(err))?)
                }
            } else {
                match src[0] {
                    0 => None,
                    1 => Some(T::decode(&src[1..], ptr_decoder)
                                .map_err(|err| OptionDecodeError::Some(err))?),
                    x => return Err(OptionDecodeError::Discriminant(x)),
                }
            }
        )
    }
}

pub enum OptionDecodeError<T: Verbatim<P>, P> {
    Discriminant(u8),
    NonZeroPadding,
    Some(T::Error),
}

impl<P, T: Verbatim<P>> fmt::Debug for OptionDecodeError<T, P>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OptionDecodeError::Discriminant(d) => f.debug_tuple("Discriminant")
                                                   .field(&d)
                                                   .finish(),
            OptionDecodeError::NonZeroPadding => f.debug_tuple("NonZeroPadding")
                                                  .finish(),
            OptionDecodeError::Some(err) => f.debug_tuple("Some")
                                             .field(&err)
                                             .finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::num::NonZeroU32;

    #[test]
    fn option_u32() {
        assert_eq!(<Option<u32> as Verbatim>::NONZERO_NICHE, false);
        assert_eq!(<Option<u32> as Verbatim>::LEN, 5);

        let n = 0x1234_5678_u32;
        let mut opt = Some(n);

        assert_eq!(opt.encode(Vec::<u8>::new(), &mut ()).unwrap(),
                   [0x01, // Discriminant
                    0x78, 0x56, 0x34, 0x12]);

        opt.take();
        assert_eq!(opt.encode(Vec::<u8>::new(), &mut ()).unwrap(),
                   [0x00, // Discriminant
                    0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn nonzero_u32() {
        assert_eq!(<Option<NonZeroU32> as Verbatim>::NONZERO_NICHE, false);
        assert_eq!(<Option<NonZeroU32> as Verbatim>::LEN, 4);

        let n = NonZeroU32::new(0x1234_5678_u32).unwrap();
        let mut opt = Some(n);

        assert_eq!(opt.encode(Vec::<u8>::new(), &mut ()).unwrap(),
                   [0x78, 0x56, 0x34, 0x12]);

        opt.take();
        assert_eq!(opt.encode(Vec::<u8>::new(), &mut ()).unwrap(),
                   [0x00, 0x00, 0x00, 0x00]);
    }
}
