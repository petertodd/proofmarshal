use std::ptr::NonNull;
use std::sync::Arc;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Heap(NonNull<()>);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeapAllocator;

impl Heap {
    fn alloc<T>(value: T::Owned) -> Self
        where T: ?Sized + Type<Heap>
    {
        let p = Arc::into_raw(Arc::new(value));

        Heap(NonNull::new(p as *mut T::Owned as *mut ())
                     .expect("Arc raw pointer to be non-null"))
    }

    unsafe fn clone_ptr<T>(&self) -> Own<T, Self>
        where T: ?Sized + Type<Heap>
    {
        // Safe to create orig_t as it won't be dropped.
        let orig_t: ManuallyDrop<Arc<T::Owned>>  = ManuallyDrop::new(Arc::from_raw(self.0.cast::<T::Owned>().as_ptr()));

        let cloned_t: Arc<T::Owned> = Arc::clone(&*orig_t);

        let ptr = Heap(NonNull::new(Arc::into_raw(cloned_t) as *mut T::Owned as *mut ())
                               .expect("Arc raw pointer to be non-null"));
        Own::from_raw(ptr)
    }

    unsafe fn dealloc<T>(self)
        where T: ?Sized + Type<Heap>
    {
        let _ = self.take::<T>();
    }

    unsafe fn get<T>(&self) -> &T::Type
        where T: ?Sized + Type<Heap>
    {
        let r: &T::Owned = self.get_owned::<T>();
        let r: &T::Type = r.borrow();

        // safe as we own the thing we're borrowing from
        &*(r as *const T::Type)
    }

    unsafe fn take<T>(self) -> Arc<T::Owned>
        where T: ?Sized + Type<Heap>
    {
        let this = ManuallyDrop::new(self);
        Arc::from_raw(this.0.as_ptr().cast())
    }

    unsafe fn get_owned<T>(&self) -> &T::Owned
        where T: ?Sized + Type<Heap>
    {
        &*self.0.cast().as_ptr()
    }

    unsafe fn get_mut<'p, T>(&'p mut self) -> &'p mut T::Owned
        where T: ?Sized + Type<Heap>
    {
        // Safe to create orig_t as it won't be dropped.
        let mut orig_t: ManuallyDrop<Arc<T::Owned>>  = ManuallyDrop::new(Arc::from_raw(self.0.cast::<T::Owned>().as_ptr()));

        let ref_t: &mut T::Owned = Arc::make_mut(&mut *orig_t);

        // Extend the lifetime.
        //
        // This is safe as we own the Arc
        &mut *(ref_t as *mut _)
    }
}

impl Ptr for Heap {
    type Error = !;
    type Allocator = HeapAllocator;

    unsafe fn clone_ptr<T: ?Sized + Type<Self>>(&self) -> Own<T, Self> {
        self.clone_ptr::<T>()
    }
    unsafe fn dealloc<T: ?Sized + Type<Self>>(self) {
        self.dealloc::<T>()
    }
    fn allocator() -> HeapAllocator {
        HeapAllocator
    }
}

impl TryGet for Heap {
    unsafe fn try_get<'p,T>(&'p self) -> Result<Ref<'p,T,Heap>, !>
        where T: ?Sized + Type<Self>
    {
        Ok(Ref::Borrowed(self.get::<T>()))
    }
    unsafe fn try_take<'p, T>(self) -> Result<T::Owned, !>
        where T: ?Sized + Type<Self>
    {
        let boxed = self.take::<T>();

        Ok(Arc::try_unwrap(boxed)
               .unwrap_or_else(|boxed| (&*boxed).clone()))
    }

}

impl TryGetMut for Heap {
    unsafe fn try_get_mut<'p, T>(&'p mut self) -> Result<&'p mut T::Owned, Self::Error>
        where T: ?Sized + Type<Self>
    {
        Ok(self.get_mut::<T>())
    }
}

impl Get for Heap {
    unsafe fn get<'p, T>(&'p self) -> Ref<'p,T,Heap>
        where T: ?Sized + Type<Self>
    {
        self.try_get::<T>().unwrap()
    }
    unsafe fn take<'p, T>(self) -> T::Owned
        where T: ?Sized + Type<Self>
    {
        self.try_take::<T>().unwrap()
    }

}

impl GetMut for Heap {
    unsafe fn get_owned<T>(&self) -> &T::Owned
        where T: ?Sized + Type<Self>
    {
        self.get_owned::<T>()
    }

    unsafe fn get_mut<T>(&mut self) -> &mut T::Owned
        where T: ?Sized + Type<Self>
    {
        self.try_get_mut::<T>().unwrap()
    }
}

impl Default for Heap {
    fn default() -> Heap {
        panic!("shouldn't be called directly")
    }
}

impl Alloc for HeapAllocator {
    type Ptr = Heap;

    fn alloc<T>(&mut self, value: T::Owned) -> Own<T, Heap>
        where T: ?Sized + Type<Heap>
    {
        let ptr = Heap::alloc::<T>(value);

        unsafe { Own::from_raw(ptr) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_primitives() {
        let own: Own<u8, Heap> = HeapAllocator.alloc(42u8);
    }

    #[test]
    fn unsized_primitives() {
        let mut own: Own<[u8], Heap> = HeapAllocator.alloc::<[u8]>(vec![42;1]);

        own.get_mut().push(11);

        assert_eq!(own.get().len(), 2);

        let v: Vec<u8> = own.take();
    }
}
