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

pub enum EncodeOwnedPtrState<T: ?Sized + Save<Z>, Z: Zone> {
    /// Initial state; `encode_poll()` has not been called.
    Initial,

    /// We have a value that needs encoding.
    Value(T::State),

    /// We've finished encoding the value (or never needed too) and now have a pointer that needs
    /// encoding.
    Ptr(<Z::Ptr as Ptr>::Persist),
}

unsafe impl<T, P, Z> Encode<Z> for OwnedPtr<T,P>
where Z: Zone<Ptr=P>,
      P: Ptr,
      T: ?Sized + Save<Z>,
{
    const BLOB_LAYOUT: BlobLayout = <<Z::Ptr as Ptr>::Persist as Encode<Z>>::BLOB_LAYOUT
                                        .extend(<T::Metadata as Primitive>::BLOB_LAYOUT);

    type State = EncodeOwnedPtrState<T, Z>;

    fn init_encode_state(&self) -> Self::State {
        EncodeOwnedPtrState::Initial
    }

    fn encode_poll<D: Dumper<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Pending> {
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

impl<T, P, Z> Decode<Z> for OwnedPtr<T,P>
where Z: Zone<Ptr=P>,
      P: Ptr,
      T: ?Sized + Load<Z>,
{
    type Error = LoadOwnedPtrError<<<Z::Ptr as Ptr>::Persist as Primitive>::Error, <T::Metadata as Primitive>::Error>;

    type ValidateChildren = OwnedPtrValidator<T,Z>;

    fn validate_blob<'a>(blob: Blob<'a, Self, Z>) -> Result<BlobValidator<'a, Self, Z>, Self::Error> {
        // There's no bound guaranteeing that Z::PersistPtr outlives 'a, so create a new blob with
        // a shorter lifetime to do the decoding.
        let blob2 = Blob::<Self, Z>::new(&blob[..], ()).unwrap();
        let mut fields = blob2.validate_struct();

        // If we hadn't done that, the following line would create a Blob<'a, Z::PersistPtr, Z>,
        // and fail with a "<Z as Zone>::PersistPtr may not live long enough" error.
        let raw = fields.field_blob::<<Z::Ptr as Ptr>::Persist>();
        let raw = <<Z::Ptr as Ptr>::Persist as Primitive>::validate_blob(raw).map_err(LoadOwnedPtrError::Ptr)?;
        let raw = <<Z::Ptr as Ptr>::Persist as Primitive>::decode_blob(raw);

        // Metadata doesn't have this issue as it's always valid for the 'static lifetime.
        let metadata = fields.field_blob::<T::Metadata>();
        let metadata = <T::Metadata as Primitive>::validate_blob(metadata).map_err(LoadOwnedPtrError::Metadata)?;
        let metadata = <T::Metadata as Primitive>::decode_blob(metadata);

        let fatptr = FatPtr::<T,_> { raw, metadata };
        Ok(blob.assume_valid(OwnedPtrValidator::FatPtr(fatptr)))
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Z>, loader: &impl LoadPtr<Z>) -> Self {
        let mut fields = blob.decode_struct(loader);

        let fatptr = FatPtr {
            raw: fields.field::<<Z::Ptr as Ptr>::Persist>().into(),
            metadata: fields.field(),
        };
        fields.assert_done();

        unsafe {
            Self::new_unchecked(ValidPtr::new_unchecked(fatptr))
        }
    }
}

pub enum OwnedPtrValidator<T: ?Sized + Load<Z>, Z: Zone> {
    FatPtr(FatPtr<T, <Z::Ptr as Ptr>::Persist>),
    Value(T::ValidateChildren),
    Done,
}

impl<T: ?Sized + Load<Z>, Z: Zone> ValidateChildren<Z> for OwnedPtrValidator<T,Z> {
    fn validate_children<V>(&mut self, ptr_validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>
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
