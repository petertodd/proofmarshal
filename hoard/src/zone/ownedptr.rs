use super::*;

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;

use crate::marshal::*;
use crate::marshal::blob::*;
use crate::coerce::TryCastRef;

/// An owned pointer.
///
/// Extends`ValidPtr` with ownership semantics, acting like it owns a `T` value and properly
/// deallocating the pointer on drop.
#[repr(transparent)]
pub struct OwnedPtr<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<Box<T>>,
    inner: ManuallyDrop<ValidPtr<T,P>>,
}

unsafe impl<T: ?Sized + Pointee, P: Ptr> NonZero for OwnedPtr<T,P>
where P: NonZero {}

unsafe impl<T: ?Sized + Pointee, P: Ptr, Q: Ptr> TryCastRef<OwnedPtr<T,Q>> for OwnedPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&OwnedPtr<T,Q>, Self::Error> {
        self.inner.try_cast_ref()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}

impl<T: ?Sized + Pointee, P: Ptr> ops::Deref for OwnedPtr<T,P> {
    type Target = ValidPtr<T,P>;

    fn deref(&self) -> &ValidPtr<T,P> {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr> ops::DerefMut for OwnedPtr<T,P> {
    fn deref_mut(&mut self) -> &mut ValidPtr<T,P> {
        &mut self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr> OwnedPtr<T,P> {
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
    pub unsafe fn new_unchecked(ptr: ValidPtr<T,P>) -> Self {
        Self {
            marker: PhantomData,
            inner: ManuallyDrop::new(ptr),
        }
    }

    /// Unwraps the inner `ValidPtr`.
    ///
    /// The value is *not* deallocated! It is the callee's responsibility to do that; failing to do
    /// so may leak memory.
    pub fn into_inner(self) -> ValidPtr<T,P> {
        let mut this = ManuallyDrop::new(self);

        unsafe { (&mut *this.inner as *mut ValidPtr<T,P>).read() }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for OwnedPtr<T,P> {
    fn drop(&mut self) {
        let this = unsafe { core::ptr::read(self) };
        P::dealloc_owned(this)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Clone for OwnedPtr<T,P>
where T: Clone,
      P: Clone
{
    fn clone(&self) -> Self {
        P::clone_ptr(self)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for OwnedPtr<T,P>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        P::fmt_debug_own(self, f)
    }
}

/*
impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for OwnedPtr<T,P>
where P: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&*self.inner, f)
    }
}
*/



#[derive(Debug)]
pub enum EncodeOwnedPtrState<T, P> {
    /// Initial state; `encode_poll()` has not been called.
    Initial,

    /// We have a value that needs encoding.
    Value(T),

    /// We've finished encoding the value (or never needed too) and now have a pointer that needs
    /// encoding.
    Ptr(P),
}

/*
unsafe impl<T, P> Encode<P> for OwnedPtr<T,P>
where P: Ptr,
      T: ?Sized + Save<P>,
{
    const BLOB_LAYOUT: BlobLayout = <P::Persist as Encode<P>>::BLOB_LAYOUT
                                        .extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type State = EncodeOwnedPtrState<T::State, P::Persist>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnedPtrState::Initial
    }

    fn encode_poll<D: Dumper<P>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Pending> {
        loop {
            match state {
                EncodeOwnedPtrState::Initial => {
                    *state = match dumper.try_save_ptr(self) {
                        Ok(ptr) => EncodeOwnedPtrState::Ptr(ptr),
                        Err(value) => EncodeOwnedPtrState::Value(value.init_save_state()),
                    };
                },
                EncodeOwnedPtrState::Value(value_state) => {
                    let value = dumper.try_save_ptr(self).expect_err("dumper try_save_ptr() inconsistent");
                    let (d, persist_ptr) = value.save_poll(value_state, dumper)?;
                    dumper = d;

                    *state = EncodeOwnedPtrState::Ptr(persist_ptr);
                },
                EncodeOwnedPtrState::Ptr(ptr) => break ptr.encode_poll(&mut (), dumper),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeOwnedPtrState::Ptr(ptr) = state {
            dst.write_primitive(ptr)?
               .write_primitive(&self.metadata)?
               .finish()
        } else {
            panic!("encode_blob() called prior to encode_poll() finishing")
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub enum LoadOwnedPtrError<P,M> {
    Ptr(P),
    Metadata(M),
}

impl<T, P> Decode<P> for OwnedPtr<T, P>
where P: Ptr,
      T: ?Sized + Load<P>,
{
    type Error = LoadOwnedPtrError<<P::Persist as Primitive>::Error, <T::Metadata as Primitive>::Error>;

    type ValidateChildren = OwnedPtrValidator<T, P>;

    fn validate_blob<'a>(blob: Blob<'a, Self, P>) -> Result<BlobValidator<'a, Self, P>, Self::Error> {
        let mut fields = blob.validate_struct();
        let inner = fields.primitive_field::<FatPtr<T, P::Persist>>().unwrap();
        Ok(fields.done(OwnedPtrValidator::FatPtr(*inner)))
    }
}

pub enum OwnedPtrValidator<T: ?Sized + Load<P>, P: Ptr> {
    FatPtr(FatPtr<T, P::Persist>),
    Value(T::ValidateChildren),
    Done,
}

impl<T: ?Sized + Load<P>, P: Ptr> ValidateChildren<P> for OwnedPtrValidator<T, P> {
    fn validate_children<V>(&mut self, ptr_validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>
    {
        loop {
            match self {
                Self::Done => break Ok(()),
                Self::FatPtr(fatptr) => {
                    *self = match ptr_validator.validate_ptr(fatptr)? {
                        Some(blob_validator) => Self::Value(blob_validator.into_state()),
                        None => Self::Done,
                    };
                },
                Self::Value(value) => {
                    let _: () = value.validate_children(ptr_validator)?;
                    *self = Self::Done;
                }
            }
        }
    }
}

/*
impl<T: ?Sized + Load<Q>, P: Decode<Q>, Q> fmt::Debug for OwnedPtrValidator<T,P,Q>
where P: Ptr + fmt::Debug,
      T::ValidateChildren: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::FatPtr(fat) => f.debug_tuple("FatPtr").field(&fat).finish(),
            Self::Value(value) => f.debug_tuple("Value").field(&value).finish(),
            Self::Done => f.debug_tuple("Done").finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_size() {
        /*
        assert_eq!(mem::size_of::<<
            (OwnedPtr<(OwnedPtr<u8,!>, OwnedPtr<OwnedPtr<u8,!>,!>), !>, OwnedPtr<u8,!>)
            as Decode<!>>::ValidateChildren>(), 3);
        */
    }
}
*/
*/