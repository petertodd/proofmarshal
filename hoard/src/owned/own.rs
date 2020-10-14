use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::ptr;

#[repr(transparent)]
pub struct Own<'a, T: 'a + ?Sized> {
    marker: PhantomData<&'a T>,
    ptr: NonNull<T>,
}

impl<'a, T: 'a + ?Sized> Own<'a, T> {
    pub unsafe fn new_unchecked(owned: &'a mut T) -> Self {
        Self {
            marker: PhantomData,
            ptr: NonNull::from(owned),
        }
    }

    pub fn leak(this: Self) -> &'a mut T {
        let this = ManuallyDrop::new(this);
        unsafe {
            &mut *this.ptr.as_ptr()
        }
    }
}

impl<T: ?Sized> Drop for Own<'_, T> {
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.ptr.as_ptr()) }
    }
}

impl<T: ?Sized> Deref for Own<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            &*self.ptr.as_ptr()
        }
    }
}

impl<T: ?Sized> DerefMut for Own<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            &mut *self.ptr.as_ptr()
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Own<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
