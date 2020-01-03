use super::*;

use std::any::type_name;
use std::cmp;
use std::convert::identity;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::mem;
use std::ops;


use crate::pointee::Pointee;

use crate::marshal::blob::*;
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::load::*;
use crate::marshal::save::*;
use crate::marshal::{PtrValidator, Dumper};

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
        //Z::fmt_debug_valid_ptr(self, f)
        todo!()
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Pointer for ValidPtr<T, Z>
where Z::Ptr: fmt::Pointer
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> ValidateBlob for ValidPtr<T, Z>
where T::Metadata: ValidateBlob
{
    type Error = <FatPtr<T,Z> as ValidateBlob>::Error;

    fn validate<'a, V: PaddingValidator>(mut blob: BlobCursor<'a, Self, V>)
        -> Result<ValidBlob<'a, Self>, BlobError<Self::Error, V::Error>>
    {
        blob.field::<FatPtr<T,Z>,_>(identity)?;
        unsafe { blob.assume_valid() }
    }
}


unsafe impl<T: ?Sized + PersistPointee, Z: Zone> Persist for ValidPtr<T, Z> {
    type Persist = ValidPtr<T::Persist, Z::Persist>;
    type Error = <ValidPtr<T::Persist, Z::Persist> as ValidateBlob>::Error;
}

#[derive(Debug)]
pub enum ValidateState<'a, T: ?Sized, S> {
    Initial,
    Value {
        value: &'a T,
        state: S,
    },
}


unsafe impl<'a, Z: Zone, T: ?Sized + Pointee> ValidateChildren<'a, Z> for ValidPtr<T, Z>
where T: ValidatePointeeChildren<'a, Z>
{
    type State = ValidateState<'a, T::Persist, T::State>;

    fn validate_children(_: &'a Self::Persist) -> Self::State {
        ValidateState::Initial
    }

    fn poll<V: PtrValidator<Z>>(this: &'a Self::Persist, state: &mut Self::State, validator: &V) -> Result<(), V::Error> {
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
                        None => break Ok(()),
                    }
                },
                ValidateState::Value { value, state } => {
                    T::poll(value, state, validator)?;

                    break Ok(())
                },
            }
        }
    }
}

impl<Z: Zone, T: ?Sized + Load<Z>> Decode<Z> for ValidPtr<T,Z> {
}

impl<T: ?Sized + Pointee, Z: Zone, Y: Zone> Encoded<Y> for ValidPtr<T,Z>
where T: Saved<Y>
{
    type Encoded = ValidPtr<T::Saved, Y>;
}

#[derive(Debug)]
pub enum EncodeState<'a, T: ?Sized + Save<'a, Y>, Z: Zone, Y: Zone> {
    /// Initial state; `encode_poll()` has not been called.
    Initial(&'a ValidPtr<T, Z>),

    /// We have a value that needs encoding.
    Value {
        value: &'a T,
        state: T::State,
    },

    /// We've finished encoding the value (or never needed too) and now have a pointer that needs
    /// encoding.
    Done(Y::PersistPtr),
}

impl<'a, T: 'a + ?Sized + Pointee, Z: 'a + Zone, Y: Zone> Encode<'a, Y> for ValidPtr<T,Z>
where T: Save<'a, Y>,
      Z: SavePtr<Y>,
{
    type State = EncodeState<'a, T, Z, Y>;

    fn make_encode_state(&'a self) -> Self::State {
        EncodeState::Initial(self)
    }

    fn encode_poll<D: Dumper<Y>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        loop {
            *state = match state {
                EncodeState::Initial(this) => {
                    match Z::try_save_ptr(this, &dumper) {
                        Ok(raw) => EncodeState::Done(raw),
                        Err(value) => EncodeState::Value { state: value.make_save_state(), value },
                    }
                },
                EncodeState::Value { value, state } => {
                    let (d, blob_ptr) = value.save_poll(state, dumper)?;
                    dumper = d;
                    EncodeState::Done(D::blob_ptr_to_zone_ptr(blob_ptr))
                },
                EncodeState::Done(_) => {
                    break Ok(dumper)
                }
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeState::Done(raw) = state {
            dst.write_primitive(raw)?
               .write_primitive(&self.metadata)?
               .finish()
        } else {
            panic!("<{} as Encode<{}>>::encode_blob() called before child encoding finished",
                   type_name::<Self>(), type_name::<Y>())
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
