//! Zone references.

use core::ops;

use owned::Owned;

use crate::zone::{Ptr, Alloc};

#[derive(Debug)]
#[repr(C)]
pub struct Own<T: ?Sized, P: Ptr> {
    pub zone: P::Zone,
    pub this: T,
}

impl<T: ?Sized, P: Ptr> Own<T,P> {
    pub fn into<U>(self) -> U
        where T: Into<U>
    {
        self.this.into()
    }

    pub fn as_ref<'a>(&'a self) -> Ref<'a, T, P> {
        Ref {
            this: &self.this,
            zone: self.zone,
        }
    }

    pub fn as_mut<'a>(&'a mut self) -> RefMut<'a, T, P> {
        RefMut {
            this: &mut self.this,
            zone: self.zone,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Ref<'a, T: ?Sized, P: Ptr> {
    pub this: &'a T,
    pub zone: P::Zone,
}

#[derive(Debug)]
#[repr(C)]
pub struct RefMut<'a, T: ?Sized, P: Ptr> {
    pub this: &'a mut T,
    pub zone: P::Zone,
}

pub enum CowRef<'a, T: ?Sized + Owned, P: Ptr> {
    Borrowed(Ref<'a, T, P>),
    Owned(Own<T::Owned, P>),
}

// --- From conversions ---
impl<'a, T, P: Ptr> From<T> for Own<T, P>
where P: Default
{
    fn from(this: T) -> Self {
        Self {
            this,
            zone: P::allocator().zone(),
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<&'a T> for Ref<'a, T, P>
where P: Default
{
    fn from(this: &'a T) -> Self {
        Self {
            this,
            zone: P::allocator().zone(),
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<&'a mut T> for Ref<'a, T, P>
where P: Default
{
    fn from(this: &'a mut T) -> Self {
        Self {
            this,
            zone: P::allocator().zone(),
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<RefMut<'a, T, P>> for Ref<'a, T, P> {
    fn from(r: RefMut<'a, T, P>) -> Self {
        Self {
            this: r.this,
            zone: r.zone,
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<&'a mut T> for RefMut<'a, T, P>
where P: Default
{
    fn from(this: &'a mut T) -> Self {
        Self {
            this,
            zone: P::allocator().zone(),
        }
    }
}

// ---- Deref impls ------
impl<T: ?Sized, P: Ptr> ops::Deref for Own<T,P> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.this
    }
}

impl<T: ?Sized, P: Ptr> ops::DerefMut for Own<T,P> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.this
    }
}

impl<'a, T: ?Sized, P: Ptr> ops::Deref for Ref<'a, T, P> {
    type Target = &'a T;
    fn deref(&self) -> &&'a T {
        &self.this
    }
}

impl<'a, T: ?Sized, P: Ptr> ops::Deref for RefMut<'a, T, P> {
    type Target = Ref<'a, T, P>;
    fn deref(&self) -> &Ref<'a, T, P> {
        // Safe b/c Ref and RefMut are #[repr(C)] with same layout
        unsafe { &*(self as *const Self as *const Ref<T,P>) }
    }
}

// --- Clone and Copy impls ---
impl<T: Clone, P: Ptr> Clone for Own<T,P> {
    fn clone(&self) -> Self {
        Self {
            this: self.this.clone(),
            zone: self.zone,
        }
    }
}
impl<T: Copy, P: Ptr> Copy for Own<T,P> {}

impl<'a, T: ?Sized, P: Ptr> Clone for Ref<'a,T,P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized, P: Ptr> Copy for Ref<'a,T,P> {}
