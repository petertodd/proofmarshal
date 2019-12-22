use super::*;

macro_rules! unsafe_impl_identity_cast {
    ($( $t:ty, )*) => {$(
        unsafe impl TryCastRef<$t> for $t {
            type Error = !;

            #[inline(always)]
            fn try_cast_ref(&self) -> Result<&Self, !> {
                Ok(self)
            }
        }

        unsafe impl TryCast<$t> for $t {
            #[inline(always)]
            fn try_cast(self) -> Result<Self, !> where Self: Sized {
                Ok(self)
            }
        }

        unsafe impl TryCastMut<$t> for $t {
            #[inline(always)]
            fn try_cast_mut(&mut self) -> Result<&mut Self, !> {
                Ok(self)
            }
        }
    )*}
}

unsafe_impl_identity_cast! {
    (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}
