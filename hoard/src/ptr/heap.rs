use std::ptr::NonNull;
use std::alloc::Layout;
use std::mem;

use super::*;

#[derive(Debug)]
pub struct Heap {
    raw: NonNull<()>,
}

impl Default for Heap {
    #[inline]
    fn default() -> Self {
        panic!()
    }
}

impl Heap {
    #[inline]
    unsafe fn heap_dealloc(ptr: NonNull<()>, layout: Layout) {
        if layout.size() > 0 {
            std::alloc::dealloc(ptr.cast().as_ptr(), layout)
        }
    }

    #[inline]
    unsafe fn heap_alloc(layout: Layout) -> NonNull<()> {
        if layout.size() > 0 {
            let ptr = std::alloc::alloc(layout);
            NonNull::new(ptr.cast())
                    .unwrap_or_else(|| std::alloc::handle_alloc_error(layout))
        } else {
            NonNull::new_unchecked(layout.align() as *mut ())
        }
    }
}

impl From<!> for Heap {
    #[inline]
    fn from(never: !) -> Self {
        never
    }
}

impl Ptr for Heap {
    type Zone = ();
    type Clean = !;
    type Blob = !;

    #[inline]
    fn from_clean(never: !) -> Self {
        match never {}
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        let r = self.try_get_dirty_mut::<T>(metadata).into_ok().trust();
        let layout = Layout::for_value(r);
        std::ptr::drop_in_place::<T>(r);
        Self::heap_dealloc(NonNull::from(r).cast(), layout)
    }

    #[inline(always)]
    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<MaybeValid<&T>, Self::Clean> {
        let ptr = T::make_fat_ptr_mut(self.raw.as_ptr(), metadata);
        Ok((&*ptr).into())
    }

    #[inline(always)]
    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Clean> {
        let ptr = T::make_fat_ptr_mut(self.raw.as_ptr(), metadata);
        Ok((&mut *ptr).into())
    }

    unsafe fn try_take_dirty_then<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Clean>
        where F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        let ptr = T::make_fat_ptr_mut(self.raw.as_ptr(), metadata);
        let src: &mut T = &mut *(ptr as *mut _);
        let layout = Layout::for_value(src);

        struct DeallocOnDrop {
            ptr: NonNull<()>,
            layout: Layout,
        }

        impl Drop for DeallocOnDrop {
            #[inline(always)]
            fn drop(&mut self) {
                unsafe { Heap::heap_dealloc(self.ptr, self.layout) }
            }
        }

        let dropper = DeallocOnDrop { ptr: self.raw, layout };
        let src: RefOwn<T> = RefOwn::new_unchecked(src);

        Ok(f(src.into()))
    }

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
        src.take_unsized(|src| {
            let metadata = T::metadata(&*src);
            let layout = Layout::for_value::<T>(&*src);

            unsafe {
                let dst = Self::heap_alloc(layout);
                std::ptr::copy_nonoverlapping::<u8>(
                    &*src as *const T as *const u8,
                    dst.as_ptr().cast(),
                    layout.size()
                );
                mem::forget(src);

                Bag::from_raw_parts(Self { raw: dst }, metadata)
            }
        })
    }
}

impl TryGet for Heap {
    type Error = !;

    #[inline(always)]
    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: Pointee + IntoOwned
    {
        self.try_get_dirty::<T>(metadata)
            .map(|r| Ref::Borrowed(r.trust()).into())
    }

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: Pointee + IntoOwned,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        self.try_take_dirty_then(metadata, f)
    }
}

impl TryGetMut for Heap {
    #[inline(always)]
    unsafe fn try_get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Error>
        where T: Pointee + IntoOwned
    {
        self.try_get_dirty_mut::<T>(metadata)
    }
}

impl Get for Heap {
    #[inline(always)]
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: Pointee + IntoOwned
    {
        let r = self.try_get_dirty::<T>(metadata).into_ok();
        Ref::Borrowed(r.trust()).into()
    }

    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: Pointee + IntoOwned,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        self.try_take_dirty_then(metadata, f).into_ok()
    }
}

impl GetMut for Heap {
    #[inline(always)]
    unsafe fn get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> MaybeValid<&mut T>
        where T: Pointee + IntoOwned
    {
        self.try_get_dirty_mut::<T>(metadata).into_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let bag = Heap::alloc(42u8);
        assert_eq!(bag.try_get_dirty().into_ok(), &42u8);

        let n = bag.try_take_dirty().into_ok();
        assert_eq!(n, 42u8);
    }

    #[test]
    fn test_get() {
        let bag = Heap::alloc(42u8);
    }

    #[test]
    fn zero_sized_does_not_alloc() {
        let bag = Heap::alloc(());
        assert_eq!(bag.ptr().raw.as_ptr() as usize, 1);
    }
}
