use super::*;

use thiserror::Error;

unsafe impl TryCoerce<bool> for u8 {
    type Error = TryCoerceBoolError;

    #[inline(always)]
    fn try_coerce_ptr(this: &u8) -> Result<*const bool, TryCoerceBoolError> {
        match this {
            0 | 1 => Ok(this as *const u8 as *const bool),
            _ => Err(TryCoerceBoolError),
        }
    }
}

#[derive(Error,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
#[error("not a valid bool")]
pub struct TryCoerceBoolError;
