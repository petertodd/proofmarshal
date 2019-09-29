use std::ptr::NonNull;
use std::sync::Arc;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Heap(NonNull<()>);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeapAllocator;

impl Heap {
    fn alloc<T>(value: T) -> Self {
        let p = Arc::into_raw(Arc::new(value));

        Heap(NonNull::new(p as *mut T as *mut ())
                     .expect("Arc raw pointer to be non-null"))
    }

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self> {
        // Safe to create orig_t as it won't be dropped.
        let orig_t: ManuallyDrop<Arc<T>>  = ManuallyDrop::new(Arc::from_raw(self.0.cast::<T>().as_ptr()));

        let cloned_t: Arc<T> = Arc::clone(&*orig_t);

        let ptr = Heap(NonNull::new(Arc::into_raw(cloned_t) as *mut T as *mut ())
                               .expect("Arc raw pointer to be non-null"));
        Own::from_raw(ptr)
    }

    unsafe fn dealloc<T>(self) {
        let _ = self.take::<T>();
    }

    unsafe fn get<T>(&self) -> &T {
        &*self.0.cast::<T>().as_ptr()
    }

    unsafe fn take<T>(self) -> Arc<T> {
        let this = ManuallyDrop::new(self);
        Arc::from_raw(this.0.as_ptr().cast())
    }

    unsafe fn get_mut<'p, T: Clone>(&'p mut self) -> &'p mut T {
        // Safe to create orig_t as it won't be dropped.
        let mut orig_t: ManuallyDrop<Arc<T>>  = ManuallyDrop::new(Arc::from_raw(self.0.cast::<T>().as_ptr()));

        let ref_t: &mut T = Arc::make_mut(&mut *orig_t);

        // Extend the lifetime.
        //
        // This is safe as we own the Arc
        &mut *(ref_t as *mut T)
    }
}

impl Ptr for Heap {
    type Error = !;
    type Allocator = HeapAllocator;

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self> {
        self.clone_ptr::<T>()
    }
    unsafe fn dealloc<T>(self) {
        self.dealloc::<T>()
    }
    unsafe fn try_get<'p, T: Load<Self>>(&'p self) -> Result<Cow<'p, T>, !> {
        Ok(Cow::Borrowed(self.get::<T>()))
    }
    unsafe fn try_take<'p, T: Load<Self>>(self) -> Result<T, !> {
        let boxed = self.take::<T>();

        Ok(Arc::try_unwrap(boxed)
               .unwrap_or_else(|boxed| (&*boxed).clone()))
    }

    fn allocator() -> HeapAllocator {
        HeapAllocator
    }
}

impl MutPtr for Heap {
    unsafe fn try_get_mut<'p, T: Load<Self>>(&'p mut self) -> Result<&'p mut T, Self::Error> {
        Ok(self.get_mut::<T>())
    }
}

impl Default for Heap {
    fn default() -> Heap {
        panic!("shouldn't be called directly")
    }
}

impl Alloc for HeapAllocator {
    type Ptr = Heap;

    fn alloc<T>(&mut self, value: T) -> Own<T, Heap> {
        let ptr = Heap::alloc(value);

        unsafe { Own::from_raw(ptr) }
    }
}
