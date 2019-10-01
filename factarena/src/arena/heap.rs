use core::ptr::NonNull;

use super::*;

#[derive(Debug,Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Heap;

impl Arena for Heap {
    type Ptr = NonNull<()>;

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: NonNull<()>, metadata: T::Metadata) {
        let fat = T::make_fat_ptr_mut(ptr.as_ptr(), metadata);
        Box::from_raw(fat);
    }

    #[inline(always)]
    unsafe fn debug_deref<T: ?Sized + Pointee>(ptr: &Self::Ptr, metadata: T::Metadata) -> Option<&T> {
        let p: *const T = T::make_fat_ptr(ptr.as_ptr(), metadata);
        Some(&*p)
    }

    /*
    #[inline]
    fn clone_own<T: Pointee>(&self, own: &Own<T,Self>) -> Own<T,Self>
        where T: Clone,
    {
        let r = unsafe {
            Self::debug_deref::<T>(own.ptr(), own.metadata()).unwrap()
        };
        let value = r.clone();

        unsafe {
            let ptr: *mut T = Box::into_raw(Box::new(value));
            let ptr: NonNull<T> = NonNull::new_unchecked(ptr);

            Own::from_raw(ptr.cast(), T::make_sized_metadata())
        }
    }
    */
}

impl Locate for Heap {
    type Error = !;
    type Locator = Self;
}

impl Allocate for Heap {
    type Allocator = Self;
}

impl Alloc for Heap {
    type Arena = Heap;

    fn locator(&self) -> &Self {
        self
    }

    fn alloc<T: Pointee>(&mut self, value: T) -> Own<T,Heap> {
        unsafe {
            let ptr: *mut T = Box::into_raw(Box::new(value));
            let ptr: NonNull<T> = NonNull::new_unchecked(ptr);

            Own::from_raw(ptr.cast(), T::make_sized_metadata())
        }
    }
}

impl TryGet<Heap> for Heap {
    #[inline(always)]
    fn try_get<'p, T: ?Sized + Type<Self>>(&self, own: &'p Own<T,Self>) -> Result<&'p T, !> {
        let r = unsafe {
            Self::debug_deref(own.ptr(), own.metadata()).unwrap()
        };
        Ok(r)
    }

    #[inline(always)]
    fn try_take<T: Type<Self>>(&self, own: Own<T,Self>) -> Result<T, !> {
        let (ptr, metadata) = own.into_raw();
        let fat = T::make_fat_ptr_mut(ptr.as_ptr(), metadata);
        unsafe {
            Ok(*Box::from_raw(fat))
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let owned_v: Own<u8, Heap> = Own::new(42u8);

        dbg!(owned_v);
    }
}
*/
