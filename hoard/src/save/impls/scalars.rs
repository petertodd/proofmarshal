use super::*;

use core::slice;
use core::mem;
use core::num;

use leint::Le;

impl<Z> Encoded<Z> for ! {
    type Encoded = Self;
}

impl<Z: Zone> Encode<'_, Z> for ! {
    type State = !;
    fn save_children(&self) -> Self::State { match *self {} }
    fn poll<D: Dumper<Z>>(&self, _: &mut Self::State, _: D) -> Result<D, D::Error> { match *self {} }
    fn encode_blob<'a, W: WriteBlob>(&'a self, _: &!, _: W) -> Result<W::Ok, W::Error> { match *self {} }
}

impl<Z> Encoded<Z> for bool {
    type Encoded = Self;
}

impl<Z: Zone> Encode<'_, Z> for bool {
    type State = ();

    #[inline(always)]
    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&[if *self { 1 } else { 0 }])?
           .finish()
    }

    fn save_children(&self) -> Self::State { }
    fn poll<D: Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }
}

macro_rules! unsafe_impl_all_valid {
    ($( $t:ty, )+) => {
        $(
            impl<Z> Encoded<Z> for $t {
                type Encoded = Self;
            }

            impl<Z: Zone> Encode<'_, Z> for $t {
                type State = ();

                fn save_children(&self) -> Self::State { }
                fn poll<D: Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
                    Ok(dumper)
                }

                #[inline(always)]
                fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
                    let src = unsafe { slice::from_raw_parts(self as *const _ as *const u8,
                                                             mem::size_of::<Self>()) };
                    dst.write_bytes(src)?
                        .finish()
                }
            }
        )+
    }
}

unsafe_impl_all_valid! {
    (),
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
    num::NonZeroU8, Le<num::NonZeroU16>, Le<num::NonZeroU32>, Le<num::NonZeroU64>, Le<num::NonZeroU128>,
    num::NonZeroI8, Le<num::NonZeroI16>, Le<num::NonZeroI32>, Le<num::NonZeroI64>, Le<num::NonZeroI128>,
}
