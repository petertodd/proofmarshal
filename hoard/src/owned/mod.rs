use core::borrow::Borrow;
use core::ptr;
use core::mem::ManuallyDrop;

pub mod own;
pub use self::own::Own;

pub mod take;
pub use self::take::Take;

pub mod refs;
pub use self::refs::Ref;

pub trait IntoOwned {
    type Owned : Borrow<Self> + Take<Self>;
    fn into_owned(self: Own<Self>) -> Self::Owned;
}

impl<T> IntoOwned for T {
    type Owned = Self;

    fn into_owned(self: Own<'_, Self>) -> Self::Owned {
        let this = ManuallyDrop::new(self);

        unsafe {
            ptr::read(&**this)
        }
    }
}
