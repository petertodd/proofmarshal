use super::*;

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;

use crate::marshal::*;
use crate::marshal::blob::*;

/// An owned pointer.
#[repr(C)]
pub struct Own<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<T>,
    inner: ManuallyDrop<FatPtr<T,P>>,
}

impl<T: ?Sized + Pointee, P: Ptr> ops::Deref for Own<T,P> {
    type Target = FatPtr<T,P>;

    fn deref(&self) -> &FatPtr<T,P> {
        &self.inner
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Own<T,P> {
    pub unsafe fn new_unchecked(ptr: FatPtr<T,P>) -> Self {
        Self {
            marker: PhantomData,
            inner: ManuallyDrop::new(ptr),
        }
    }

    pub fn into_inner(self) -> FatPtr<T,P> {
        let mut this = ManuallyDrop::new(self);

        unsafe { (&mut *this.inner as *mut FatPtr<T,P>).read() }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for Own<T,P> {
    fn drop(&mut self) {
        let this = unsafe { core::ptr::read(self) };
        P::dealloc_own(this)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for Own<T,P>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        P::fmt_debug_own(self, f)
    }
}

impl<T: ?Sized + Pointee, P: Ptr> fmt::Pointer for Own<T,P>
where P: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&*self.inner, f)
    }
}

pub enum EncodeOwnState<T: ?Sized + Save<Q>, P: Encode<Q>, Q> {
    Initial,
    Value(T::State),
    Ptr(P::State),
}

impl<T, P, Q> Encode<Q> for Own<T,P>
where Q: Encode<Q>,
      P: Ptr + Encode<Q>,
      T: ?Sized + Save<Q>,
{
    const BLOB_LAYOUT: BlobLayout = Q::BLOB_LAYOUT.extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type State = EncodeOwnState<T, P, Q>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnState::Initial
    }

    fn encode_poll<D: Dumper<Q>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Pending> {
        loop {
            match state {
                EncodeOwnState::Initial => {
                    *state = match P::encode_own(self) {
                        Ok(ptr_state) => EncodeOwnState::Ptr(ptr_state),
                        Err(value_state) => EncodeOwnState::Value(value_state),
                    };
                },
                EncodeOwnState::Value(value_state) => {
                    let metadata = self.metadata;
                    let (d, ptr_state) = P::encode_own_value(self, value_state, dumper)?;
                    dumper = d;

                    *state = EncodeOwnState::Ptr(ptr_state);
                },
                EncodeOwnState::Ptr(state) => break self.raw.encode_poll(state, dumper),
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        if let EncodeOwnState::Ptr(state) = state {
            dst.write(&self.raw, state)?
               .finish()
        } else {
            panic!()
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub enum LoadOwnError<P,M> {
    Ptr(P),
    Metadata(M),
}

impl<T: ?Sized + Pointee, P: Ptr, Q> Decode<Q> for Own<T,P>
where T: Load<Q>,
      P: Decode<Q>,
      Q: Encode<Q>,
{
    type Error = LoadOwnError<P::Error, <T::Metadata as Primitive>::Error>;

    type ValidateChildren = OwnValidator<T,P,Q>;
    fn validate_blob<'a>(blob: Blob<'a, Self, Q>) -> Result<BlobValidator<'a, Self, Q>, Self::Error> {
        let mut fields = blob.validate_struct();

        let raw = fields.field_blob::<P>();
        let raw = P::ptr_validate_blob(raw).map_err(LoadOwnError::Ptr)?;
        let raw = P::ptr_decode_blob(raw);

        let metadata = fields.field_blob::<T::Metadata>();
        let metadata = <T::Metadata as Primitive>::validate_blob(metadata).map_err(LoadOwnError::Metadata)?;
        let metadata = <T::Metadata as Primitive>::decode_blob(metadata);

        let fatptr = FatPtr { raw, metadata };
        Ok(fields.done(OwnValidator::FatPtr(fatptr)))
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Q>, loader: &impl LoadPtr<Q>) -> Self {
        let mut fields = blob.decode_struct(loader);
        let fatptr = fields.field();

        unsafe {
            Self::new_unchecked(fatptr)
        }
    }
}

pub enum OwnValidator<T: ?Sized + Load<Q>, P: Decode<Q>, Q> {
    FatPtr(FatPtr<T,P>),
    Value(T::ValidateChildren),
    Done,
}

impl<T: ?Sized + Load<Q>, P: Decode<Q>, Q> ValidateChildren<Q> for OwnValidator<T,P,Q>
where P: Ptr
{
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Q>
    {
        loop {
            match self {
                Self::Done => break Ok(()),
                Self::FatPtr(fatptr) => {
                    *self = match P::ptr_validate_children(fatptr, validator)? {
                        None => Self::Done,
                        Some(state) => Self::Value(state),
                    };
                },
                Self::Value(value) => {
                    let _: () = value.validate_children(validator)?;
                    *self = Self::Done;
                }
            }
        }
    }
}

impl<T: ?Sized + Load<Q>, P: Decode<Q>, Q> fmt::Debug for OwnValidator<T,P,Q>
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
            (Own<(Own<u8,!>, Own<Own<u8,!>,!>), !>, Own<u8,!>)
            as Decode<!>>::ValidateChildren>(), 3);
    }
}
