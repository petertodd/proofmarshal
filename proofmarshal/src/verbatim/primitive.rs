use super::*;

use core::marker::PhantomData;
use core::mem;

impl<T: ?Sized, P> Verbatim<P> for PhantomData<T> {
    type Error = !;

    const LEN: usize = 0;
    const NONZERO_NICHE: bool = false;

    #[inline(always)]
    fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        assert_eq!(src.len(), <Self as Verbatim<P>>::LEN);
        Ok(PhantomData)
    }

    #[inline(always)]
    fn encode<W: io::Write>(&self, dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        Ok(dst)
    }
}

impl<P> Verbatim<P> for () {
    type Error = !;

    const LEN: usize = 0;
    const NONZERO_NICHE: bool = false;

    #[inline(always)]
    fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        assert_eq!(src.len(), <Self as Verbatim<P>>::LEN);
        Ok(())
    }

    #[inline(always)]
    fn encode<W: io::Write>(&self, dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        Ok(dst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoolDecodeError(u8);

impl<P> Verbatim<P> for bool {
    type Error = BoolDecodeError;

    const LEN: usize = 1;
    const NONZERO_NICHE: bool = false;

    #[inline(always)]
    fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
        assert_eq!(src.len(), <Self as Verbatim<P>>::LEN);
        match src[0] {
            1 => Ok(true),
            0 => Ok(false),
            x => Err(BoolDecodeError(x)),
        }
    }

    #[inline(always)]
    fn encode<W: io::Write>(&self, mut dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
        dst.write_all(&[if *self { 1 } else { 0 }])?;
        Ok(dst)
    }
}

macro_rules! impl_ints {
    ($( $t:ty, )+) => {
        $(
            impl<P> Verbatim<P> for $t {
                type Error = !;
                const LEN: usize = mem::size_of::<Self>();
                const NONZERO_NICHE: bool = false;

                #[inline(always)]
                fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
                    let mut buf = [0u8; mem::size_of::<Self>()];
                    buf.copy_from_slice(src);

                    Ok(<$t>::from_le_bytes(buf))
                }

                #[inline(always)]
                fn encode<W: io::Write>(&self, mut dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
                    let buf = self.to_le_bytes();
                    dst.write_all(&buf)?;
                    Ok(dst)
                }
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroNumDecodeError<T>(PhantomData<T>);

macro_rules! impl_nonzero_ints {
    ($( $t:ty => $inner:ty; )+) => {
        $(
            impl<P> Verbatim<P> for $t {
                type Error = NonZeroNumDecodeError<$t>;
                const LEN: usize = mem::size_of::<Self>();
                const NONZERO_NICHE: bool = true;

                #[inline(always)]
                fn decode(src: &[u8], _: &mut impl PtrDecode<P>) -> Result<Self, Self::Error> {
                    let mut buf = [0u8; mem::size_of::<Self>()];
                    buf.copy_from_slice(src);

                    let inner = <$inner>::from_le_bytes(buf);

                    <$t>::new(inner).ok_or(NonZeroNumDecodeError(PhantomData))
                }

                #[inline(always)]
                fn encode<W: io::Write>(&self, mut dst: W, _: &mut impl PtrEncode<P>) -> Result<W, io::Error> {
                    let buf: [u8; mem::size_of::<Self>()] = self.get().to_le_bytes();
                    dst.write_all(&buf)?;
                    Ok(dst)
                }
            }
        )+
    }
}

use core::num::*;

impl_nonzero_ints! {
    NonZeroU8 => u8; NonZeroU16 => u16; NonZeroU32 => u32; NonZeroU64 => u64; NonZeroU128 => u128;
    NonZeroI8 => i8; NonZeroI16 => i16; NonZeroI32 => i32; NonZeroI64 => i64; NonZeroI128 => i128;
}
