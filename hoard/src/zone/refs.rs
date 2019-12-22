//! Zone references.

use core::ops;

use super::Zone;

#[derive(Debug)]
#[repr(C)]
pub struct Own<T: ?Sized, Z> {
    pub zone: Z,
    pub this: T,
}

#[derive(Debug)]
#[repr(C)]
pub struct Ref<'a, T: ?Sized, Z> {
    pub this: &'a T,
    pub zone: Z,
}

#[derive(Debug)]
#[repr(C)]
pub struct RefMut<'a, T: ?Sized, Z> {
    pub this: &'a mut T,
    pub zone: Z,
}

impl<T: ?Sized, Z: Zone> Own<T, Z> {
    pub fn into<U>(self) -> U
        where T: Into<U>
    {
        self.this.into()
    }

    pub fn as_ref<'a>(&'a self) -> Ref<'a, T, Z> {
        Ref {
            this: &self.this,
            zone: self.zone.duplicate(),
        }
    }

    pub fn as_mut<'a>(&'a mut self) -> RefMut<'a, T, Z> {
        RefMut {
            this: &mut self.this,
            zone: self.zone.duplicate(),
        }
    }
}


/*
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
            zone: P::zone(),
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<&'a T> for Ref<'a, T, P>
where P: Default
{
    fn from(this: &'a T) -> Self {
        Self {
            this,
            zone: P::zone(),
        }
    }
}

impl<'a, T: ?Sized, P: Ptr> From<&'a mut T> for Ref<'a, T, P>
where P: Default
{
    fn from(this: &'a mut T) -> Self {
        Self {
            this,
            zone: P::zone(),
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
            zone: P::zone(),
        }
    }
}
*/

// ---- Deref impls ------
impl<T: ?Sized, Z: Zone> ops::Deref for Own<T,Z> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.this
    }
}

impl<T: ?Sized, Z: Zone> ops::DerefMut for Own<T,Z> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.this
    }
}

impl<'a, T: ?Sized, Z: Zone> ops::Deref for Ref<'a, T, Z> {
    type Target = &'a T;
    fn deref(&self) -> &&'a T {
        &self.this
    }
}

impl<'a, T: ?Sized, Z: Zone> ops::Deref for RefMut<'a, T, Z> {
    type Target = Ref<'a, T, Z>;
    fn deref(&self) -> &Ref<'a, T, Z> {
        // Safe b/c Ref and RefMut are #[repr(C)] with same layout
        unsafe { &*(self as *const Self as *const Ref<T,Z>) }
    }
}

/*
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
*/
