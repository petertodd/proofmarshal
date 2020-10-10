use core::mem::ManuallyDrop;
use core::ptr;

pub unsafe trait Take<T: ?Sized> : Sized {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R;

    fn take_sized(self) -> T
        where T: Sized
    {
        self.take_unsized(|src| {
            unsafe {
                ptr::read(&**src)
            }
        })
    }
}

unsafe impl<T> Take<T> for T {
    fn take_unsized<F, R>(self, f: F) -> R
        where F: FnOnce(&mut ManuallyDrop<T>) -> R
    {
        let mut this = ManuallyDrop::new(self);
        f(&mut this)
    }
}
