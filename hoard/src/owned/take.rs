use core::mem::ManuallyDrop;
use core::ptr;

use super::RefOwn;

pub unsafe trait Take<T: ?Sized> : Sized {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(RefOwn<T>) -> R;

    fn take_sized(self) -> T
        where T: Sized
    {
        self.take_unsized(|src| {
            let src: &mut T = RefOwn::leak(src);

            unsafe {
                ptr::read(src)
            }
        })
    }
}

unsafe impl<T> Take<T> for T {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(RefOwn<T>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        let own = unsafe { RefOwn::<T>::new_unchecked(&mut this) };
        f(own)
    }
}
