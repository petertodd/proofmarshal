use super::*;

use core::cmp;
use core::fmt;
use core::hash;
use core::mem;
use core::ops;
use core::marker::PhantomData;

use crate::{
    coerce::*,
    marshal::{
        en::{Encode, Dumper, Save, SaveState},
        de::{Load, Decode, PtrValidator},
    },
};

/// Wrapper around a `FatPtr` guaranteeing that the target of the pointer is valid.
///
/// Implements `Deref<Target=FatPtr>` so the fields of the wrapped pointer are available;
/// `DerefMut` is *not* implemented because mutating the wrapper pointer could invalidate it.
#[repr(transparent)]
pub struct ValidPtr<T: ?Sized + Pointee, P>(FatPtr<T,P>);

unsafe impl<T: ?Sized + Pointee, P> NonZero for ValidPtr<T,P>
where P: NonZero {}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastRef<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCastRef<Q>
{
    type Error = P::Error;

    fn try_cast_ref(&self) -> Result<&ValidPtr<T,Q>, Self::Error> {
        self.0.try_cast_ref()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCastMut<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCastMut<Q>
{
    fn try_cast_mut(&mut self) -> Result<&mut ValidPtr<T,Q>, Self::Error> {
        self.0.try_cast_mut()
            .map(|inner| unsafe { mem::transmute(inner) })
    }
}

unsafe impl<T: ?Sized + Pointee, P, Q> TryCast<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: TryCast<Q>
{}

impl<T: ?Sized + Pointee, P> ops::Deref for ValidPtr<T,P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized + Pointee, P> ValidPtr<T,P> {
    /// Creates a new `ValidPtr` from a `FatPtr`.
    ///
    /// # Safety
    ///
    /// You are asserting that the pointer is in fact valid.
    pub unsafe fn new_unchecked(ptr: FatPtr<T,P>) -> Self {
        Self(ptr)
    }

    /// Unwraps the pointer.
    pub fn into_inner(self) -> FatPtr<T,P> {
        self.0
    }

    /// Gets mutable access to the raw pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe because changes to the raw pointer could make it invalid.
    pub unsafe fn raw_mut(&mut self) -> &mut P {
        &mut self.0.raw
    }
}

impl<T: ?Sized + Pointee, P> From<ValidPtr<T,P>> for FatPtr<T,P> {
    /// Forwards to `into_inner()`
    fn from(valid: ValidPtr<T,P>) -> Self {
        valid.into_inner()
    }
}

// standard impls
impl<T: ?Sized + Pointee, P> fmt::Debug for ValidPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ValidPtr")
            .field("raw", &self.raw)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for ValidPtr<T,P>
where P: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<ValidPtr<T,Q>> for ValidPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &ValidPtr<T,Q>) -> bool {
        &self.0 == &other.0
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<FatPtr<T,Q>> for ValidPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &FatPtr<T,Q>) -> bool {
        &self.0 == other
    }
}

impl<T: ?Sized + Pointee, P, Q> cmp::PartialEq<ValidPtr<T,Q>> for FatPtr<T,P>
where P: cmp::PartialEq<Q>
{
    fn eq(&self, other: &ValidPtr<T,Q>) -> bool {
        self == &other.0
    }
}

impl<T: ?Sized + Pointee, P> cmp::Eq for ValidPtr<T,P>
where P: cmp::Eq {}

impl<T: ?Sized + Pointee, P> Clone for ValidPtr<T,P>
where P: Clone
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: ?Sized + Pointee, P> Copy for ValidPtr<T,P>
where P: Copy {}

impl<T: ?Sized + Pointee, P> hash::Hash for ValidPtr<T,P>
where P: hash::Hash,
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

/// State used when encoding a `ValidPtr`.
#[derive(Debug)]
pub enum EncodeState<'a, T: ?Sized + Pointee + SaveState<'a, P>, P: Ptr> {
    /// Initial state; `encode_poll()` has not been called.
    Initial,

    /// We have a value that needs encoding.
    Value {
        value: &'a (),
        metadata: T::Metadata,
        value_state: T::State,
    },

    /// We've finished encoding the value (or never needed too) and now have a pointer that needs
    /// encoding.
    Ptr(P::Persist),
}

impl<'a, T, P: Ptr> SaveState<'a, P> for ValidPtr<T,P>
where T: ?Sized + Save<P>
{
    type State = EncodeState<'a, T, P>;

    fn init_save_state(&'a self) -> Self::State {
        EncodeState::Initial
    }
}

unsafe impl<T, P> Encode<P> for ValidPtr<T,P>
where P: Ptr,
      T: ?Sized + Save<P>,
{
    fn encode_poll<'a, D: Dumper<P>>(&'a self, state: &mut <Self as SaveState<'a, P>>::State, mut dumper: D)
        -> Result<D, D::Pending>
    {
        loop {
            match state {
                EncodeState::Initial => {
                    *state = match dumper.try_save_ptr(self) {
                        Ok(ptr) => EncodeState::Ptr(ptr),
                        Err(value) => EncodeState::Value {
                            value_state: value.init_save_state(),
                            metadata: T::metadata(value),

                            // SAFETY: being zero-sized, we can safely coerce anything to a &() reference.
                            value: unsafe { &*(value as *const T as *const ()) },
                        },
                    };
                },
                EncodeState::Value { value, metadata, value_state } => {
                    // SAFETY: we created value from a &'a T reference, so we can safely turn it
                    // back into one
                    let value: &'a T = unsafe { &*T::make_fat_ptr(*value, *metadata) };
                    let (d, persist_ptr) = value.save_poll(value_state, dumper)?;
                    dumper = d;

                    *state = EncodeState::Ptr(persist_ptr);
                },
                EncodeState::Ptr(_) => break Ok(dumper),
            }
        }
    }

    fn encode_blob<'a, W: WriteBlob>(&'a self, state: &<Self as SaveState<'a,P>>::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeState::Ptr(ptr) = state {
            dst.write_primitive(ptr)?
               .write_primitive(&self.metadata)?
               .finish()
        } else {
            panic!("encode_blob() called prior to encode_poll() finishing")
        }
    }
}

/*
unsafe impl<T, P> Decode<P> for ValidPtr<T, P>
where P: Ptr,
      T: ?Sized + Load<P>,
{
    type Error = super::fatptr::ValidateError<<T::Metadata as Primitive>::Error,
                                              <P::Persist as Primitive>::Error>;

    type ChildValidator = ValidPtrValidator<T, P>;

    fn validate_blob<'a>(blob: Blob<'a, Self>) -> Result<BlobValidator<'a, Self, P>, Self::Error> {
        /*
        let mut blob = blob.validate_struct();
        let inner = blob.primitive_field::<FatPtr<T, P::Persist>>()?;
        Ok((unsafe { blob.done() }, ValidateState::FatPtr(*inner)))
        */ todo!()
    }

    /*
    fn validate_poll<'a, V>(state: &mut Self::ValidateState, validator: &V) -> Result<(), V::Error>
        where V: PtrValidator<P>
    {
        loop {
            match state {
                ValidateState::Done => break Ok(()),
                ValidateState::FatPtr(fatptr) => {
                    *state = match validator.validate_ptr(fatptr)? {
                        Some(state) => ValidateState::Value(state),
                        None => ValidateState::Done,
                    };
                },
                ValidateState::Value(value_state) => {
                    T::validate_poll(value_state, validator)?;
                    *state = ValidateState::Done;
                }
            }
        }
    }
    */
}

pub enum ValidateState<T: ?Sized + Load<P>, P: Ptr> {
    FatPtr(FatPtr<T, P::Persist>),
    Value(T::ChildValidator),
    Done,
}
*/

/*
impl<T: ?Sized + Load<P>, P: Ptr> ValidateChildren<P> for OwnedPtrValidator<T, P> {
    fn validate_children<V>(&mut self, ptr_validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>
    {
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
