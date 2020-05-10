use std::ops;
use std::fmt;

use super::*;

pub struct ValidPtr<T: ?Sized + Pointee, P>(FatPtr<T, P>);

impl<T: ?Sized + Pointee, P> ValidPtr<T,P> {
    pub unsafe fn new_unchecked(fat: FatPtr<T,P>) -> Self {
        Self(fat)
    }

    pub fn into_inner(self) -> FatPtr<T,P> {
        self.0
    }
}

impl<T: ?Sized + Pointee, P> ops::Deref for ValidPtr<T, P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


impl<T: ?Sized + Pointee, P> fmt::Debug for ValidPtr<T, P>
where T: fmt::Debug,
      P: Ptr
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            self.raw.fmt_debug_valid_ptr::<T>(self.metadata, f)
        }
    }
}
