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

pub unsafe trait Persist {}

unsafe impl Persist for ! {}

pub fn encode<T: ?Sized + Save<!>>(value: &T) -> Vec<u8> {
    /*
    match value.save_poll(&mut value.init_save_state(), vec![]) {
        Ok((r, ())) => r,
        Err(never) => never,
    }
    */
    todo!()
}

pub fn decode<T: Decode<!>>(blob: &[u8]) -> Result<T, T::Error> {
    /*
    let blob = Blob::new(blob, T::make_sized_metadata()).expect("wrong size");

    let mut validator = T::validate_blob(blob)?;
    let fully_valid_blob = validator.poll(&mut ()).unwrap();
    Ok(T::decode_blob(fully_valid_blob, &()))
    */
    todo!()
}

/// A type whose values can be saved behind pointers in a zone.
pub unsafe trait Save<Z: Zone> : Pointee + Owned {
    /// Makes a blob layout from the pointer metadata.
    fn dyn_blob_layout(metadata: Self::Metadata) -> BlobLayout;

    type State;
    fn init_save_state(&self) -> Self::State;

    fn save_poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, <Z::Ptr as Ptr>::Persist), D::Pending>;
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
pub trait Load<Z: Zone> : Save<Z> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist,
    {
        todo!()
    }
}

pub trait ValidateChildren<Z: Zone> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>;
}

impl<Z: Zone> ValidateChildren<Z> for () {
    fn validate_children<V>(&mut self, _: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>
    {
        Ok(())
    }
}

/// A type that can be encoded in a zone.
pub unsafe trait Encode<Z: Zone> : Sized {
    const BLOB_LAYOUT: BlobLayout;

    type State;
    fn init_encode_state(&self) -> Self::State;

    fn encode_poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>;

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;
}

/// A type that can be decoded from a zone.
pub trait Decode<Z: Zone> : Encode<Z> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist
    {
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), blob.len());
        unsafe {
            blob.assume_valid()
        }
    }
}

unsafe impl<Z: Zone, T: Primitive> Encode<Z> for T {
    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT;

    type State = ();
    fn init_encode_state(&self) -> () { }

    fn encode_poll<D: Dumper<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending>
        where Z: Zone
    {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(dst)
    }
}

impl<Z: Zone, T: Primitive> Decode<Z> for T {
    type Error = T::Error;

    type ValidateChildren = ();
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error> {
        T::validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, _: &impl Loader<Z>) -> Self {
        T::decode_blob(blob)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, _: &impl Loader<Z>) -> Ref<'p, Self> {
        Self::load_blob(blob)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist,
    {
        Self::deref_blob(blob)
    }
}

unsafe impl<Z: Zone, T: Encode<Z>> Save<Z> for T {
    #[inline(always)]
    fn dyn_blob_layout(_: ()) -> BlobLayout {
        T::BLOB_LAYOUT
    }

    type State = T::State;
    fn init_save_state(&self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D: Dumper<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, <Z::Ptr as Ptr>::Persist), D::Pending>
        where Z: Zone
    {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.try_save_blob(Self::BLOB_LAYOUT.size(), | dst | {
            match self.encode_blob(state, dst) {
                Ok(()) => (),
                Err(never) => never,
            }
        })
    }
}

impl<Z: Zone, T: Decode<Z>> Load<Z> for T {
    type Error = T::Error;

    type ValidateChildren = T::ValidateChildren;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error> {
        T::validate_blob(blob)
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self::Owned {
        T::decode_blob(blob, loader)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        T::load_blob(blob, loader)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist
    {
        T::deref_blob(blob)
    }
}

/// Saves data to a zone.
pub trait Dumper<Z: Zone> : Sized {
    type Pending;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T,Z::Ptr>) -> Result<<Z::Ptr as Ptr>::Persist, &'p T>;

    /// Saves a blob.
    fn try_save_blob(self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, <Z::Ptr as Ptr>::Persist), Self::Pending>;
}

pub trait Loader<Z: Zone> {
    fn load_blob<'a, T: ?Sized + Load<Z>>(&self, ptr: &'a ValidPtr<T, <Z::Ptr as Ptr>::Persist>) -> FullyValidBlob<'a, T, Z>;

    fn blob_zone(&self) -> &Z;
}

impl<Z: Zone, L: Loader<Z>> Loader<Z> for &'_ L {
    fn load_blob<'a, T: ?Sized + Load<Z>>(&self, ptr: &'a ValidPtr<T, <Z::Ptr as Ptr>::Persist>) -> FullyValidBlob<'a, T, Z> {
        (*self).load_blob(ptr)
    }

    fn blob_zone(&self) -> &Z {
        (*self).blob_zone()
    }
}

/*
impl Loader<!> for () {
    fn load_blob<'a, T: ?Sized + Load<!>>(&self, ptr: &'a ValidPtr<T,!>) -> FullyValidBlob<'a, T, !> {
        match ptr.raw {}
    }

    fn blob_zone(&self) -> &! {
        panic!()
    }
}
*/

pub trait ValidatePtr<Z: Zone> {
    type Error;

    fn validate_ptr<'p, T: ?Sized + Load<Z>>(&mut self, ptr: &'p FatPtr<T, <Z::Ptr as Ptr>::Persist>)
        -> Result<Option<BlobValidator<'p, T, Z>>, Self::Error>;
}

impl ValidatePtr<!> for () {
    type Error = !;

    fn validate_ptr<'p, T: ?Sized + Load<!>>(&mut self, ptr: &'p FatPtr<T,!>)
        -> Result<Option<BlobValidator<'p, T, !>>, !>
    {
        match ptr.raw {}
    }
}

/*
impl Dumper<!> for Vec<u8> {
    type Pending = !;
    type BlobPtr = ();

    fn save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Self::BlobPtr), !> {
        let start = self.len();
        self.resize_with(start + size, u8::default);
        let dst = &mut self[start .. ];
        f(dst);
        Ok((self, ()))
    }
}
*/
