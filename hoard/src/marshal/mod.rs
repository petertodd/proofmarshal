use super::*;

use core::any::Any;
use core::convert::TryFrom;
use core::mem::{self, MaybeUninit};
use core::slice;

pub mod impls;

pub mod blob;
use self::blob::*;

mod primitive;
pub use self::primitive::*;

/// Marker for types that can be mem-mapped.
pub unsafe trait Persist {}

unsafe impl Persist for ! {}

/// A type whose values can be saved behind pointers in a zone.
pub unsafe trait Save<P: Ptr> : Pointee + Owned {
    /// Makes a blob layout from the pointer metadata.
    fn dyn_blob_layout(metadata: Self::Metadata) -> BlobLayout;

    type State;
    fn init_save_state(&self) -> Self::State;

    fn save_poll<D: Dumper<P>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, P::Persist), D::Pending>;
}

pub trait Error : 'static + Any + fmt::Debug + Send {
    fn type_name(&self) -> &'static str;
}

impl<E: ?Sized + 'static + Any + fmt::Debug + Send> Error for E {
    fn type_name(&self) -> &'static str {
        core::any::type_name::<E>()
    }
}

/// A type whose values can be loaded from pointers in a zone.
pub trait Load<P: Ptr> : Save<P> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<P>;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist,
    {
        let actual_size = blob.len();
        let r = unsafe { blob.assume_valid() };
        assert_eq!(mem::size_of_val(&r), actual_size);
        r
    }
}

pub trait ValidateChildren<P: Ptr> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>;
}

impl<P: Ptr> ValidateChildren<P> for () {
    fn validate_children<V>(&mut self, _: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>
    {
        Ok(())
    }
}

/// A type that can be encoded in a zone.
pub unsafe trait Encode<P: Ptr> : Sized {
    const BLOB_LAYOUT: BlobLayout;

    type State;
    fn init_encode_state(&self) -> Self::State;

    fn encode_poll<D: Dumper<P>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>;

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;
}

/// A type that can be decoded from a zone.
pub trait Decode<P: Ptr> : Encode<P> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<P>;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), blob.len());
        unsafe {
            blob.assume_valid()
        }
    }
}

unsafe impl<P: Ptr, T: Primitive> Encode<P> for T {
    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT;

    type State = ();
    fn init_encode_state(&self) -> () { }

    fn encode_poll<D: Dumper<P>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending>
        where P: Ptr
    {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(dst)
    }
}

impl<P: Ptr, T: Primitive> Decode<P> for T {
    type Error = T::Error;

    type ValidateChildren = ();
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        T::validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl Loader<P>) -> Self {
        T::decode_blob(blob)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl Loader<P>) -> Ref<'p, Self> {
        Self::load_blob(blob)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist,
    {
        Self::deref_blob(blob)
    }
}

unsafe impl<P: Ptr, T: Encode<P>> Save<P> for T {
    #[inline(always)]
    fn dyn_blob_layout(_: ()) -> BlobLayout {
        T::BLOB_LAYOUT
    }

    type State = T::State;
    fn init_save_state(&self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D: Dumper<P>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, P::Persist), D::Pending> {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.try_save_blob(Self::BLOB_LAYOUT.size(), | dst | {
            match self.encode_blob(state, dst) {
                Ok(()) => (),
                Err(never) => never,
            }
        })
    }
}

impl<P: Ptr, T: Decode<P>> Load<P> for T {
    type Error = T::Error;

    type ValidateChildren = T::ValidateChildren;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        T::validate_blob(blob)
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self::Owned {
        T::decode_blob(blob, loader)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
        T::load_blob(blob, loader)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        T::deref_blob(blob)
    }
}

/// Saves data to a zone.
pub trait Dumper<P: Ptr> : Sized {
    type Pending;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, P>) -> Result<P::Persist, &'p T>;

    /// Saves a blob.
    fn try_save_blob(self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, P::Persist), Self::Pending>;
}

pub trait Loader<P: Ptr> {
    fn load_blob<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T, P::Persist>) -> FullyValidBlob<'a, T, P>;
}

impl<P: Ptr, L: Loader<P>> Loader<P> for &'_ L {
    fn load_blob<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T, P::Persist>) -> FullyValidBlob<'a, T, P> {
        (*self).load_blob(ptr)
    }
}

pub trait ValidatePtr<P: Ptr> {
    type Error;

    fn validate_ptr<'p, T: ?Sized + Load<P>>(&mut self, ptr: &'p FatPtr<T, P::Persist>)
        -> Result<Option<BlobValidator<'p, T, P>>, Self::Error>;
}

impl ValidatePtr<!> for () {
    type Error = !;

    fn validate_ptr<'p, T: ?Sized + Load<!>>(&mut self, ptr: &'p FatPtr<T,!>)
        -> Result<Option<BlobValidator<'p, T, !>>, !>
    {
        match ptr.raw {}
    }
}
