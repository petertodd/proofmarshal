use core::num;

use leint::Le;

pub mod prelude {
    pub use crate::blob::*;
    pub use crate::load::*;
    pub use crate::save::*;
}

use crate::load::*;
use crate::save::*;

pub trait Primitive : 'static + Sized + Persist<Persist=Self> + Decode<!> + for<'a> Encode<'a, !> + ValidateBlob
{
    fn save_to_vec(&self) -> Vec<u8> {
        todo!()
    }
}

/// Implements `Primitive` for one or more types.
#[macro_export]
macro_rules! impl_primitive {
    ($($t:ty,)+ $(,)?) => {$(
        impl Primitive for $t {}
    )+}
}

impl_primitive! {
    !, (), bool,
    u8, Le<u16>, Le<u32>, Le<u64>, Le<u128>,
    i8, Le<i16>, Le<i32>, Le<i64>, Le<i128>,
}
