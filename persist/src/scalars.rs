use super::*;

use core::mem;
use core::slice;

macro_rules! impl_all_valid_scalar {
    ($( $t:ty, )+) => {
        $(
            impl Persist for $t {
                #[inline(always)]
                fn write_canonical<W: Write>(&self, mut dst: W) -> io::Result<W> {
                    let buf = unsafe { slice::from_raw_parts(self as *const _ as *const u8,
                                                             mem::size_of::<Self>()) };
                    dst.write_all(buf)?;
                    Ok(dst)
                }
            }

            impl<V: ?Sized> Validate<V> for $t {
                type Error = !;

                #[inline(always)]
                fn validate<'a>(maybe: MaybeValid<'a, Self>, _validator: &mut V) -> Result<Valid<'a, Self>, Self::Error> {
                    unsafe { Ok(maybe.assume_valid()) }
                }
            }
        )+
    }
}

impl_all_valid_scalar! {
    (), u8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateBoolError(u8);

impl Persist for bool {
    #[inline(always)]
    fn write_canonical<W: Write>(&self, mut dst: W) -> io::Result<W> {
        dst.write_all(&[*self as u8])?;
        Ok(dst)
    }
}

impl<V: ?Sized> Validate<V> for bool {
    type Error = ValidateBoolError;

    #[inline(always)]
    fn validate<'a>(maybe: MaybeValid<'a, Self>, _validator: &mut V) -> Result<Valid<'a, Self>, Self::Error> {
        match maybe[0] {
            0 | 1 => unsafe { Ok(maybe.assume_valid()) },
            x => Err(ValidateBoolError(x)),
        }
    }
}
