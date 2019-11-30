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

impl<T: ?Sized + Pointee, P: Ptr> OwnedPtr<T,P> {

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

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for OwnedPtr<T,P>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        P::fmt_debug_own(self, f)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for OwnedPtr<T,P>
where P: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&*self.inner, f)
    }
}

pub enum EncodeOwnedPtrState<T: ?Sized + Save<Z>, P: Encode<Z>, Z> {
    Initial,
    Value(T::State),
    Ptr(P::State),
}

unsafe impl<T, P, Z> Encode<Z> for OwnedPtr<T,P>
where Z: BlobZone,
      P: Ptr + Encode<Z>,
      T: ?Sized + Save<Z>,
{
    fn blob_layout() -> BlobLayout {
        <Z::BlobPtr as Encode<Z>>::blob_layout().extend(<T::Metadata as Primitive>::BLOB_LAYOUT)
    }

    type State = EncodeOwnedPtrState<T, P, Z>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnedPtrState::Initial
    }

    fn encode_poll<D: SavePtr<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Pending> {
        loop {
            match state {
                EncodeOwnedPtrState::Initial => {
                    *state = match P::encode_own(self) {
                        Ok(ptr_state) => EncodeOwnedPtrState::Ptr(ptr_state),
                        Err(value_state) => EncodeOwnedPtrState::Value(value_state),
                    };
                },
                EncodeOwnedPtrState::Value(value_state) => {
                    let metadata = self.metadata;
                    let (d, ptr_state) = P::encode_own_value(self, value_state, dumper)?;
                    dumper = d;

                    *state = EncodeOwnedPtrState::Ptr(ptr_state);
                },
                EncodeOwnedPtrState::Ptr(state) => break self.raw.encode_poll(state, dumper),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeOwnedPtrState::Ptr(state) = state {
            dst.write(&self.raw, state)?
               .finish()
        } else {
            panic!()
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub enum LoadOwnedPtrError<P,M> {
    Ptr(P),
    Metadata(M),
}

impl<T: ?Sized + Pointee, P: Ptr, Z> Decode<Z> for OwnedPtr<T,P>
where T: Load<Z>,
      P: Decode<Z>,
      Z: BlobZone,
{
    type Error = LoadOwnedPtrError<P::Error, <T::Metadata as Primitive>::Error>;

    type ValidateChildren = OwnedPtrValidator<T,P,Z>;
    fn validate_blob<'a>(blob: Blob<'a, Self, Z>) -> Result<BlobValidator<'a, Self, Z>, Self::Error>
        where Z: BlobZone
    {
        /*
        let mut fields = blob.validate_struct();

        let raw = fields.field_blob::<P>();
        let raw = P::ptr_validate_blob(raw).map_err(LoadOwnedPtrError::Ptr)?;
        let raw = P::ptr_decode_blob(raw);

        let metadata = fields.field_blob::<T::Metadata>();
        let metadata = <T::Metadata as Primitive>::validate_blob(metadata).map_err(LoadOwnedPtrError::Metadata)?;
        let metadata = <T::Metadata as Primitive>::decode_blob(metadata);

        let fatptr = FatPtr { raw, metadata };
        Ok(fields.done(OwnedPtrValidator::FatPtr(fatptr)))
        */
        todo!()
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Z>, loader: &impl LoadPtr<Z>) -> Self {
        let mut fields = blob.decode_struct(loader);
        let fatptr = fields.field();

        unsafe {
            Self::new_unchecked(ValidPtr::new_unchecked(fatptr))
        }
    }
}

pub enum OwnedPtrValidator<T: ?Sized + Load<Q>, P: Decode<Q>, Q> {
    FatPtr(FatPtr<T,P>),
    Value(T::ValidateChildren),
    Done,
}

impl<T: ?Sized + Load<Z>, P: Decode<Z>, Z> ValidateChildren<Z> for OwnedPtrValidator<T,P,Z>
where P: Ptr
{
    fn validate_children<V>(&mut self, ptr_validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>, Z: BlobZone
    {
        /*
        loop {
            match self {
                Self::Done => break Ok(()),
                Self::FatPtr(fatptr) => {
                    *self = match P::ptr_validate_children(fatptr, ptr_validator)? {
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
        */
        todo!()
    }
}

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
        assert_eq!(mem::size_of::<<
            (OwnedPtr<(OwnedPtr<u8,!>, OwnedPtr<OwnedPtr<u8,!>,!>), !>, OwnedPtr<u8,!>)
            as Decode<!>>::ValidateChildren>(), 3);
    }
}
