use super::*;

use core::any::type_name;
use core::convert::identity;
use core::borrow::Borrow;
use core::borrow::BorrowMut;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;
use core::ptr;

use nonzero::NonZero;

use crate::blob::*;
use crate::load::*;
use crate::save::*;

/// An owned pointer.
///
/// Extends`ValidPtr` with ownership semantics, acting like it owns a `T` value and properly
/// deallocating the pointer on drop.
#[repr(transparent)]
pub struct OwnedPtr<T: ?Sized + Pointee, Z: Zone> {
    marker: PhantomData<Box<T>>,
    inner: ManuallyDrop<ValidPtr<T, Z>>,
}

unsafe impl<T: ?Sized + Pointee, Z: Zone> NonZero for OwnedPtr<T, Z> {}

/*
unsafe impl<T: ?Sized + Pointee, P: Ptr, Q: Ptr> TryCastRef<OwnedPtr<T,Q>> for OwnedPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&OwnedPtr<T,Q>, Self::Error> {
        self.inner.try_cast_ref()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}
*/

impl<T: ?Sized + Pointee, Z: Zone> ops::Deref for OwnedPtr<T, Z> {
    type Target = ValidPtr<T, Z>;

    fn deref(&self) -> &ValidPtr<T, Z> {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, Z: Zone> ops::DerefMut for OwnedPtr<T, Z> {
    fn deref_mut(&mut self) -> &mut ValidPtr<T, Z> {
        &mut self.inner
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Borrow<ValidPtr<T, Z>> for OwnedPtr<T, Z> {
    fn borrow(&self) -> &ValidPtr<T, Z> {
        self
    }
}

impl<T: ?Sized + Pointee, Z: Zone> BorrowMut<ValidPtr<T, Z>> for OwnedPtr<T, Z> {
    fn borrow_mut(&mut self) -> &mut ValidPtr<T, Z> {
        self
    }
}

impl<T: ?Sized + Pointee, Z: Zone> OwnedPtr<T, Z> {
/*
    pub fn new(value: impl Take<T>) -> Self
        where P: Default
    {
        P::allocator().alloc(value)
    }
*/

    /// Creates a new `OwnedPtr` from a `ValidPtr`.
    ///
    /// # Safety
    ///
    /// The `ValidPtr` must point to a uniquely owned value that can be safely dropped via
    /// `Ptr::dealloc_owned()`.
    pub unsafe fn new_unchecked(ptr: ValidPtr<T, Z>) -> Self {
        Self {
            marker: PhantomData,
            inner: ManuallyDrop::new(ptr),
        }
    }

    /// Unwraps the inner `ValidPtr`.
    ///
    /// The value is *not* deallocated! It is the callee's responsibility to do that; failing to do
    /// so may leak memory.
    pub fn into_inner(self) -> ValidPtr<T, Z> {
        let mut this = ManuallyDrop::new(self);

        unsafe { (&mut *this.inner as *mut ValidPtr<T, Z>).read() }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Drop for OwnedPtr<T, Z> {
    fn drop(&mut self) {
        unsafe {
            let this = ptr::read(self);
            Z::try_take_dirty_unsized(this, |this| {
                match this {
                    Ok(value) => ptr::drop_in_place(value),
                    Err(_persist_ptr) => (),
                }
            })
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Clone for OwnedPtr<T, Z>
where T: Clone, Z: Clone
{
    fn clone(&self) -> Self {
        Z::clone_ptr(self)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for OwnedPtr<T, Z>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Z::fmt_debug_valid_ptr(self, f)
    }
}


impl<T: ?Sized + PersistPtr, Z: Zone> Persist for OwnedPtr<T, Z> {
    type Persist = OwnedPtr<T::Persist, Z::Persist>;
}

impl<Z: Zone, T: ?Sized + Pointee + ValidateBlob> ValidateBlob for OwnedPtr<T, Z> {
    type Error = <ValidPtr<T,Z> as ValidateBlob>::Error;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<ValidPtr<T,Z>,_>(identity)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<'a, Z: Zone, T: ?Sized + Pointee> ValidateChildren<'a, Z> for OwnedPtr<T, Z>
where T: ValidatePtrChildren<'a, Z>
{
    type State = super::validptr::ValidateState<'a, T::Persist, T::State>;

    fn validate_children(this: &'a OwnedPtr<T::Persist, Z::Persist>) -> Self::State {
        <ValidPtr<T,Z> as ValidateChildren<'a, Z>>::validate_children(this)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        <ValidPtr<T,Z> as ValidateChildren<'a, Z>>::poll(this, state, validator)?;
        Ok(unsafe { mem::transmute(this) })
    }
}

impl<Z: Zone, T: ?Sized + Load<Z>> Decode<Z> for OwnedPtr<T,Z> {
}

/*
impl<Y: Zone, Z: Zone, T: ?Sized + Saved<Y>> Encoded<Y> for OwnedPtr<T, Z> {
    type Encoded = OwnedPtr<T::Saved, Y>;
}

impl<'a, Y: Zone, Z: 'a + Zone + Encode<'a, Y>, T: 'a + ?Sized + Save<'a, Y>> Encode<'a, Y> for OwnedPtr<T, Z> {
    type State = <ValidPtr<T, Z> as Encode<'a, Y>>::State;

    fn save_children(&'a self) -> Self::State {
        Encode::save_children(&*self.inner)
    }

    fn poll<D: Dumper<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        Encode::poll(&*self.inner, state, dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        self.inner.encode_blob(state, dst)
    }
}
*/
