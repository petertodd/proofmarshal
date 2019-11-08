use core::mem::ManuallyDrop;

/// Wrapper representing a value that may have been dropped.
#[repr(transparent)]
pub struct MaybeDropped<T: ?Sized>(ManuallyDrop<T>);

impl<T> MaybeDropped<T> {
    /// Constructs a new `MaybeDropped<T>` by **dropping** a value.
    pub fn drop(value: T) -> Self {
        let mut value = ManuallyDrop::new(value);
        unsafe { ManuallyDrop::drop(&mut value) };
        Self(value)
    }

    /// Constructs a new `MaybeDropped<T>` by **forgetting** a value.
    pub const fn forget(value: T) -> Self {
        Self(ManuallyDrop::new(value))
    }
}

impl<T: ?Sized> MaybeDropped<T> {
    pub fn from_ref(r: &T) -> &MaybeDropped<T> {
        unsafe {
            &*(r as *const T as *const Self)
        }
    }

    /// Returns a raw pointer to the underlying value.
    pub fn as_ptr(&self) -> *const T {
        self as *const _ as *const T
    }

    /// Returns a mutable raw pointer to the underlying value.
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self as *mut _ as *mut T
    }

    /// Gets a reference to the underlying value.
    ///
    /// # Safety
    ///
    /// This is unsafe because value may have already been dropped.
    pub unsafe fn get_unchecked(&self) -> &T {
        &*self.as_ptr()
    }

    /// Gets a mutable reference to the underlying value.
    ///
    /// # Safety
    ///
    /// This is unsafe because value may have already been dropped.
    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        &mut *self.as_mut_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_ptr() {
        let mut maybe = MaybeDropped::drop(42u8);
        assert_eq!(maybe.as_ptr(), &maybe as *const MaybeDropped<u8> as *const u8);
        assert_eq!(maybe.as_mut_ptr(), &mut maybe as *mut MaybeDropped<u8> as *mut u8);
    }

    #[test]
    fn from_ref() {
        let v = 42u8;
        let r = &v;

        let m = MaybeDropped::from_ref(r);

        assert_eq!(m.as_ptr(), r as *const u8);
    }
}
