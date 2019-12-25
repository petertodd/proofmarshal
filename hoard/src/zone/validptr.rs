use super::*;

use core::convert::identity;
use core::cmp;
use core::fmt;
use core::hash;
use core::mem;
use core::ops;
use core::marker::PhantomData;

use nonzero::NonZero;

use crate::blob::*;
use crate::load::*;
use crate::save::*;

/// Wrapper around a `FatPtr` guaranteeing that the target of the pointer is valid.
///
/// Implements `Deref<Target=FatPtr>` so the fields of the wrapped pointer are available;
/// `DerefMut` is *not* implemented because mutating the wrapper pointer could invalidate it.
#[repr(transparent)]
pub struct ValidPtr<T: ?Sized + Pointee, Z: Zone>(FatPtr<T, Z>);


impl<T: ?Sized + Pointee, Z: Zone> ops::Deref for ValidPtr<T, Z> {
    type Target = FatPtr<T, Z>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl<T: ?Sized + Pointee, Z: Zone> NonZero for ValidPtr<T, Z> {}

impl<T: ?Sized + Pointee, Z: Zone> ValidPtr<T, Z> {
    /// Creates a new `ValidPtr` from a `FatPtr`.
    ///
    /// # Safety
    ///
    /// You are asserting that the pointer is in fact valid.
    pub unsafe fn new_unchecked(ptr: FatPtr<T, Z>) -> Self {
        Self(ptr)
    }

    /// Gets mutable access to the raw pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe because changes to the raw pointer could make it invalid.
    pub unsafe fn raw_mut(&mut self) -> &mut Z::Ptr {
        &mut self.0.raw
    }

    pub fn into_inner(self) -> FatPtr<T,Z> {
        self.0
    }
}

// standard impls
impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for ValidPtr<T, Z>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Z::fmt_debug_valid_ptr(self, f)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Pointer for ValidPtr<T, Z>
where Z::Ptr: fmt::Pointer
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

impl<T: ?Sized + Persist, Z: Zone> Persist for ValidPtr<T, Z> {
    type Persist = ValidPtr<T::Persist, Z::Persist>;
    type Error = <FatPtr<T,Z> as Persist>::Error;

    fn validate_blob<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();
        blob.field::<FatPtr<T,Z>,_>(identity)?;
        unsafe { blob.assume_valid() }
    }
}

pub enum ValidateState<'a, T: ?Sized + Pointee, S> {
    Initial,
    Value {
        value: &'a T,
        state: S,
    },
}

unsafe impl<'a, Z: Zone, T: ?Sized + Pointee> Validate<'a, Z> for ValidPtr<T, Z>
where T: Validate<'a, Z>
{
    type State = ValidateState<'a, T::Persist, T::State>;

    fn validate_children(_: &'a ValidPtr<T::Persist, Z::Persist>) -> Self::State {
        ValidateState::Initial
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<&'a Self, V::Error> {
        loop {
            *state = match state {
                ValidateState::Initial => {
                    match validator.validate_ptr::<T>(this)? {
                        Some(value) => {
                            ValidateState::Value {
                                state: T::validate_children(value),
                                value,
                            }
                        },
                        None => break Ok(unsafe { mem::transmute(this) }),
                    }
                },
                ValidateState::Value { value, state } => {
                    T::poll(value, state, validator)?;

                    break Ok(unsafe { mem::transmute(this) })
                },
            }
        }
    }
}

impl<Z: Zone, T: ?Sized + Load<Z>> Decode<Z> for ValidPtr<T,Z> {
}

/// State used when saving a `ValidPtr`.
#[derive(Debug)]
pub enum SaveState<'a, T: ?Sized + Save<'a, Y>, Z: Zone, Y: Zone> {
    /// Initial state; `encode_poll()` has not been called.
    Initial(&'a ValidPtr<T, Z>),

    /// We have a value that needs encoding.
    Value {
        value: &'a T,
        state: T::State,
    },

    /// We've finished encoding the value (or never needed too) and now have a pointer that needs
    /// encoding.
    Done(FatPtr<T::Saved, Y::Persist>),
}

impl<T: ?Sized + Pointee, Z: Zone, Y: Zone> Encoded<Y> for ValidPtr<T,Z>
where T: Saved<Y>
{
    type Encoded = ValidPtr<T::Saved, Y>;
}

impl<'a, T: 'a + ?Sized + Pointee, Z: 'a + Zone, Y: Zone> Encode<'a, Y> for ValidPtr<T, Z>
where T: Save<'a, Y>,
      Z: SavePtr<Y>,
{
    type State = SaveState<'a, T, Z, Y>;

    fn save_children(&'a self) -> Self::State {
        SaveState::Initial(self)
    }

    fn poll<D: Dumper<Y>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        loop {
            *state = match state {
                SaveState::Initial(this) => {
                    match Z::try_save_ptr(this, &dumper) {
                        Ok(raw) => SaveState::Done(
                                        FatPtr {
                                            metadata: this.metadata,
                                            raw,
                                        }
                                   ),
                        Err(value) => SaveState::Value { state: value.save_children(), value },
                    }
                },
                SaveState::Value { value, state } => {
                    dumper = value.poll(state, dumper)?;

                    let (d, ptr) = value.save_blob(state, dumper)?;
                    dumper = d;

                    SaveState::Done(ptr)
                },
                SaveState::Done(_) => break Ok(dumper),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let SaveState::Done(ptr) = state {
            dst.write_primitive(&ptr.raw)?
               .write_primitive(&ptr.metadata)?
               .finish()
        } else {
            panic!("encode_blob() called before child encoding finished")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}
