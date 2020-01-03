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


use crate::marshal::*;
use crate::marshal::blob::*;
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::load::*;
use crate::marshal::save::*;

/// An owned pointer.
///
/// Extends`ValidPtr` with ownership semantics, acting like it owns a `T` value and properly
/// deallocating the pointer on drop.
#[repr(transparent)]
pub struct OwnedPtr<T: ?Sized + Pointee, Z: Zone> {
    marker: PhantomData<Box<T>>,
    inner: ManuallyDrop<ValidPtr<T, Z>>,
}

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
    pub fn new(value: impl Take<T>) -> Self
        where Z: Default
    {
        Z::alloc(value)
    }

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
                    Ok(value) => {
                        // value is a &mut ManuallyDrop<T>, so we need to coerce it first or
                        // drop_in_place won't actually do anything
                        let value: &mut T = value;
                        ptr::drop_in_place(value)
                    }
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


impl<T: ?Sized + Pointee, Z: Zone> ValidateBlob for OwnedPtr<T, Z>
where T::Metadata: ValidateBlob,
{
    type Error = <ValidPtr<T, Z> as ValidateBlob>::Error;

    fn validate<'a, V: PaddingValidator>(
        mut blob: BlobCursor<'a, Self, V>,
    ) -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
    {
        blob.field::<FatPtr<T,Z>,_>(identity)?;
        unsafe { blob.assume_valid() }
    }
}

unsafe impl<T: ?Sized + PersistPointee, Z: Zone> Persist for OwnedPtr<T, Z> {
    type Persist = OwnedPtr<T::Persist, Z::Persist>;
    type Error = <OwnedPtr<T::Persist, Z::Persist> as ValidateBlob>::Error;
}

unsafe impl<'a, Z: Zone, T: ?Sized + Pointee> ValidateChildren<'a, Z> for OwnedPtr<T, Z>
where T: ValidatePointeeChildren<'a, Z>
{
    type State = super::validptr::ValidateState<'a, T::Persist, T::State>;

    fn validate_children(this: &'a OwnedPtr<T::Persist, Z::Persist>) -> Self::State {
        <ValidPtr<T,Z> as ValidateChildren<'a, Z>>::validate_children(this)
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
        <ValidPtr<T,Z> as ValidateChildren<'a, Z>>::poll(this, state, validator)?;
        Ok(())
    }
}

impl<Z: Zone, T: ?Sized + Load<Z>> Decode<Z> for OwnedPtr<T,Z> {
}

impl<T: ?Sized + Pointee, Z: Zone, Y: Zone> Encoded<Y> for OwnedPtr<T,Z>
where T: Saved<Y>
{
    type Encoded = OwnedPtr<T::Saved, Y>;
}

impl<'a, T: 'a + ?Sized + Pointee, Z: 'a + Zone, Y: Zone> Encode<'a, Y> for OwnedPtr<T,Z>
where T: Save<'a, Y>,
      Z: SavePtr<Y>,
{
    type State = super::validptr::EncodeState<'a, T, Z, Y>;

    fn make_encode_state(&'a self) -> Self::State {
        self.inner.make_encode_state()
    }

    fn encode_poll<D: Dumper<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Error> {
        self.inner.encode_poll(state, dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        self.inner.encode_blob(state, dst)
    }
}

/*
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
