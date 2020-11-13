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

unsafe impl<T> Take<[T]> for Vec<T> {
    fn take_unsized<F, R>(mut self, f: F) -> R
        where F: FnOnce(RefOwn<[T]>) -> R
    {
        let slice: *mut [T] = self.as_mut_slice();

        // SAFETY: by setting the length to zero we ensure that Vec::drop won't drop the slice
        // contents.
        unsafe {
            self.set_len(0);
            f(RefOwn::new_unchecked(&mut *slice))
        }
    }
}
