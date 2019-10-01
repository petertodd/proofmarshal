//! Stack allocated arenas.

use core::alloc::Layout;
use core::cell::{RefCell, Cell};
use core::cmp;
use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull};

use super::*;

/// Stack arena.
///
/// This is what actually owns the memory of the stack arena.
#[derive(Debug)]
pub struct Stack {
    chunks: RefCell<Vec<Box<[u8]>>>,

    start: Cell<*mut u8>,
    end: Cell<*mut u8>,
}

impl<'a> Arena for &'a Stack {
    type Ptr = Ptr<'a>;

    unsafe fn dealloc<T: ?Sized + Pointee>(raw: Self::Ptr, metadata: T::Metadata) {
        let p: *mut T = T::make_fat_ptr_mut(raw.into_mut_ptr(), metadata);
        core::ptr::drop_in_place(p);
    }

    #[inline(always)]
    unsafe fn debug_deref<T: ?Sized + Pointee>(ptr: &Self::Ptr, metadata: T::Metadata) -> Option<&T> {
        let p: *const T = T::make_fat_ptr(ptr.as_ptr(), metadata);

        Some(&*p)
    }
}

impl<'a> super::Locate for &'a Stack {
    type Error = !;
    type Locator = StackLocator<'a>;
}

impl<'a> super::Allocate for &'a Stack {
    type Allocator = StackAllocator<'a>;
}

/// Locator for values in a `&'a Stack` arena.
///
/// Zero-sized.
#[derive(Debug,Clone,Copy)]
pub struct StackLocator<'a> {
    marker: PhantomData<&'a Stack>,
}

impl<'a> StackLocator<'a> {
    pub fn new(stack: &'a Stack) -> Self {
        let _ = stack;
        Self { marker: PhantomData }
    }
}

/// Allocator for a `&'a Stack` arena.
#[derive(Debug,Clone,Copy)]
pub struct StackAllocator<'a> {
    locator: StackLocator<'a>,
    stack: &'a Stack,
}

impl<'a> StackAllocator<'a> {
    fn new(stack: &'a Stack) -> Self {
        Self {
            locator: StackLocator::new(stack),
            stack,
        }
    }
}

impl<'a> TryGet<&'a Stack> for StackLocator<'a> {
    fn try_get<'p, T: ?Sized + Type<&'a Stack>>(&self, own: &'p Own<T,&'a Stack>) -> Result<&'p T, !> {
        let r: &T = unsafe {
            &*T::make_fat_ptr(own.ptr().as_ptr(), own.metadata())
        };

        Ok(r)
    }

    fn try_take<T: Type<&'a Stack>>(&self, own: Own<T,&'a Stack>) -> Result<T, !> {
        let (ptr, metadata) = own.into_raw();

        let p: *mut T = T::make_fat_ptr_mut(ptr.into_mut_ptr(), metadata);

        unsafe {
            Ok(p.read())
        }
    }
}

impl<'a> Alloc for StackAllocator<'a> {
    type Arena = &'a Stack;

    #[inline]
    fn locator(&self) -> &StackLocator<'a> {
        &self.locator
    }

    #[inline]
    fn alloc<T: Type<Self::Arena>>(&mut self, value: T) -> Own<T,Self::Arena> {
        let metadata = value.ptr_metadata();

        let ptr: *mut T = self.stack.alloc_for_layout(T::layout(metadata)).cast();

        unsafe {
            ptr.write(value);

            Own::from_raw(Ptr::new(ptr as *mut ()), metadata)
        }
    }
}


impl Stack {
    /// Creates a new stack arena.
    ///
    /// Specifically, this creates an *anchor*.
    pub fn new() -> Self {
        Self {
            chunks: RefCell::new(vec![]),
            start: Cell::new(ptr::null_mut()),
            end: Cell::new(ptr::null_mut())
        }
    }

    /// Creates a new allocator for this stack arena.
    ///
    /// Note how the lifetime is the lifetime of the *reference*.
    pub fn allocator<'a>(&'a self) -> impl Alloc<Arena=&'a Stack> {
        StackAllocator::new(self)
    }
}

/// Raw pointer type.
///
/// Acts like it owns a `&'a Stack`.
#[derive(Debug)]
pub struct Ptr<'a> {
    marker: PhantomData<&'a Stack>,
    raw: NonNull<()>,
}

impl Ptr<'_> {
    #[inline]
    unsafe fn new(raw: *mut ()) -> Self {
        Self {
            marker: PhantomData,
            raw: NonNull::new_unchecked(raw),
        }
    }
    #[inline]
    fn as_ptr(&self) -> *const () {
        self.raw.as_ptr()
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut () {
        self.raw.as_ptr()
    }

    #[inline]
    fn into_mut_ptr(mut self) -> *mut () {
        let r = self.as_mut_ptr();
        mem::forget(self);
        r
    }
}

impl Stack {
    fn double(&self, required: usize) {
        let mut chunks = self.chunks.borrow_mut();

        let prev_capacity = chunks.last().map_or(0, |chunk| chunk.len());
        let new_capacity = cmp::max(prev_capacity + 1, required).next_power_of_two();

        let mut new_chunk = vec![0; new_capacity].into_boxed_slice();

        let start = new_chunk.as_mut_ptr();

        chunks.push(new_chunk);

        unsafe {
            self.start.set(start);
            self.end.set(start.offset(new_capacity as isize))
        }
    }

    fn alloc_for_layout(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() < mem::align_of::<u64>(),
                "FIXME");

        loop {
            let remaining = (self.end.get() as usize) - (self.start.get() as usize);
            let padding = self.start.get().align_offset(layout.align());

            assert!(padding < layout.align());

            let required = padding + layout.size();

            if remaining < required {
                self.double(required);
            } else {
                let start = self.start.get();

                unsafe {
                    self.start.set(start.offset(required as isize));

                    break start.offset(padding as isize)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let stack = Stack::new();

        let mut allocator = stack.allocator();

        for i in 0u8 .. 10u8 {
            let _ = Own::new_in(i, &mut allocator);
        }

        let owned = Own::new_in(42u8, &mut allocator);
        let _r = allocator.locator().try_get(&owned).unwrap();
    }
}
