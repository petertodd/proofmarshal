//! Volatile, in-memory, zone allocation.

use core::ptr::NonNull;

use super::*;

#[derive(Default,Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Heap;

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Raw(NonNull<()>);

impl Raw {
    #[inline]
    fn from_box<T: ?Sized>(b: Box<T>) -> Self {
        let nn = Box::into_raw_non_null(b);
        Self(nn.cast())
    }

    #[inline]
    unsafe fn into_box<T: ?Sized + Pointee>(self, metadata: T::Metadata) -> Box<T> {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr_mut(thin, metadata);
        Box::from_raw(fat)
    }

    #[inline]
    unsafe fn get<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> &T {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr(thin, metadata);

        &*fat
    }

    #[inline]
    unsafe fn get_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> &mut T {
        let thin = self.0.as_ptr();
        let fat = T::make_fat_ptr_mut(thin, metadata);

        &mut *fat
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct Allocator;

impl Zone for Heap {
    type Ptr = Raw;
    type Allocator = Allocator;
    type Error = !;

    #[inline]
    fn allocator() -> Self::Allocator { Allocator }

    fn clone_rec<T: Clone>(r: &Rec<T,Self>) -> Rec<T,Self> {
        /*
        let orig = unsafe { ptr.raw.get::<T>(()) };
        let cloned = orig.clone();
        let ptr = Ptr::from_box(Box::new(cloned));

        unsafe { Unique::from_raw_parts(ptr, ()) }
        */
        unimplemented!()
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: super::Ptr<T,Self>) {
        ptr.raw.into_box::<T>(ptr.metadata);
    }

    fn fmt_debug_rec<T: ?Sized + Pointee>(rec: &Rec<T,Self>, f: &mut fmt::Formatter) -> fmt::Result
        where T: fmt::Debug
    {
        let value = unsafe { rec.ptr.raw.get::<T>(rec.ptr.metadata) };
        fmt::Debug::fmt(value, f)
    }
}

impl Alloc for Allocator {
    type Zone = Heap;

    #[inline]
    fn alloc<T>(&mut self, value: T) -> Rec<T,Self::Zone> {
        let raw = Raw::from_box(Box::new(self));
        unsafe { Rec::from_ptr(super::Ptr { raw, metadata: () }) }
    }

    #[inline]
    fn zone(&self) -> Self::Zone { Heap }
}

impl TryGet for Heap {
    fn try_get<'p, T: ?Sized + Load<Self>>(&self, r: &'p Rec<T,Self>) -> Result<Ref<'p, T, Self>, !> {
        let r: &T = unsafe { r.ptr().raw.get::<T>(r.ptr().metadata) };
        Ok(Ref::Borrowed(r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Foo(u8,u16);

    #[test]
    fn test() {
        //let bag = Bag::<_,Heap>::new(Foo(8,16));
        //dbg!(bag.get());
    }
}
