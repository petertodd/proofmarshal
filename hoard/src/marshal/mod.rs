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
    match value.save_poll(&mut value.init_save_state(), vec![]) {
        Ok((r, ())) => r,
        Err(never) => never,
    }
}

pub fn decode<T: Decode<!>>(blob: &[u8]) -> Result<T, T::Error> {
    let blob = Blob::new(blob, T::make_sized_metadata()).expect("wrong size");

    let mut validator = T::validate_blob(blob)?;
    let fully_valid_blob = validator.poll(&mut ()).unwrap();
    Ok(T::decode_blob(fully_valid_blob, &()))
}

/// A type whose values can be saved behind pointers in a zone.
pub unsafe trait Save<Z> : Pointee + Owned {
    /// Makes a blob layout from the pointer metadata.
    fn dyn_blob_layout(metadata: Self::Metadata) -> BlobLayout
        where Z: BlobZone;

    type State;
    fn init_save_state(&self) -> Self::State;

    fn save_poll<D: SavePtr<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending>
        where Z: BlobZone;
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
pub trait Load<Z> : Save<Z> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>
        where Z: BlobZone;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Self::Owned
        where Z: BlobZone;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Ref<'p, Self>
        where Z: BlobZone
    {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist, Z: BlobZone
    {
        todo!()
    }
}

pub trait ValidateChildren<Z> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>, Z: BlobZone;
}

impl<Z> ValidateChildren<Z> for () {
    fn validate_children<V>(&mut self, _: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<Z>, Z: BlobZone
    {
        Ok(())
    }
}

/// A type that can be encoded in a zone.
pub unsafe trait Encode<Z> : Sized {
    /// Returns the layout of a value of this type as a blob.
    ///
    /// Note: this would be an associated constant, except that we need the `Z: BlobZone` bound.
    fn blob_layout() -> BlobLayout
        where Z: BlobZone;

    type State;
    fn init_encode_state(&self) -> Self::State;

    fn encode_poll<D: SavePtr<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>
        where Z: BlobZone;

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>
        where Z: BlobZone;

    /// Hook for pointer encoding; non-pointer types do not need to implement this.
    ///
    /// This implements the equivalent of `init_encode_state()`, except taking a `ValidPtr` to
    /// complete the incomplete pointer type.
    ///
    /// Returns a blob pointer if the value doesn't need to be encoded. Otherwise returns the value
    /// state.
    fn ptr_init_encode_state<T: ?Sized + Pointee>(ptr: &ValidPtr<T,Self>) -> Result<Z::BlobPtr, <T as Save<Z>>::State>
        where T: Save<Z>,
              Z: BlobZone,
              Self: Ptr
    {
        unimplemented!()
    }

    /// Hook for pointer encoding; non-pointer types do not need to implement this.
    ///
    /// Encodes the value behind the pointer, returning a blob pointer when finished.
    fn ptr_encode_value_poll<T, D>(ptr: &ValidPtr<T,Self>, state: &mut T::State, dumper: D)
        -> Result<(D, Z::BlobPtr), D::Pending>
        where T: ?Sized + Save<Z>,
              D: SavePtr<Z>,
              Z: BlobZone,
              Self: Ptr
    {
        unimplemented!()
    }
}

/// A type that can be decoded from a zone.
pub trait Decode<Z> : Encode<Z> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>
        where Z: BlobZone;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Self
        where Z: BlobZone;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Ref<'p, Self>
        where Z: BlobZone
    {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist, Z: BlobZone
    {
        assert_eq!(mem::align_of::<Self>(), 1);
        assert_eq!(mem::size_of::<Self>(), blob.len());
        unsafe {
            blob.assume_valid()
        }
    }

    fn ptr_validate_blob<'a>(blob: Blob<'a, Self, Z>) -> Result<FullyValidBlob<'a, Self, Z>, Self::Error>
        where Self: Ptr, Z: BlobZone
    {
        unimplemented!()
    }

    fn ptr_decode_blob<'a>(blob: FullyValidBlob<'a, Self, Z>) -> Self
        where Self: Ptr, Z: BlobZone
    {
        unimplemented!()
    }

    fn ptr_validate_children<'p, T, V>(ptr: &'p FatPtr<T,Self>, validator: &mut V)
            -> Result<Option<BlobValidator<'p, T, Z>>, V::Error>
        where Self: Ptr, Z: BlobZone,
              T: ?Sized + Load<Z>,
              V: ValidatePtr<Z>,
    {
        unimplemented!()
    }
}

unsafe impl<Z, T> Encode<Z> for T
where T: Primitive
{
    fn blob_layout() -> BlobLayout {
        T::BLOB_LAYOUT
    }

    type State = ();
    fn init_encode_state(&self) -> () { }

    fn encode_poll<D: SavePtr<Z>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending>
        where Z: BlobZone
    {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error>
        where Z: BlobZone
    {
        self.encode_blob(dst)
    }
}

impl<Z, T> Decode<Z> for T
where T: Primitive
{
    type Error = T::Error;

    type ValidateChildren = ();
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>
        where Z: BlobZone
    {
        T::validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, _: &impl LoadPtr<Z>) -> Self
        where Z: BlobZone
    {
        T::decode_blob(blob)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, _: &impl LoadPtr<Z>) -> Ref<'p, Self>
        where Z: BlobZone
    {
        Self::load_blob(blob)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist, Z: BlobZone
    {
        Self::deref_blob(blob)
    }
}

unsafe impl<Z, T: Encode<Z>> Save<Z> for T {
    #[inline(always)]
    fn dyn_blob_layout(_: ()) -> BlobLayout where Z: BlobZone {
        T::blob_layout()
    }

    type State = T::State;
    fn init_save_state(&self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D: SavePtr<Z>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending>
        where Z: BlobZone
    {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.save_blob(Self::dyn_blob_layout(()).size(), | dst | {
            match self.encode_blob(state, dst) {
                Ok(()) => (),
                Err(never) => never,
            }
        })
    }
}

impl<Z, T: Decode<Z>> Load<Z> for T {
    type Error = T::Error;

    type ValidateChildren = T::ValidateChildren;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<BlobValidator<'p, Self, Z>, Self::Error>
        where Z: BlobZone
    {
        T::validate_blob(blob)
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Self::Owned
        where Z: BlobZone
    {
        T::decode_blob(blob, loader)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl LoadPtr<Z>) -> Ref<'p, Self>
        where Z: BlobZone
    {
        T::load_blob(blob, loader)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist, Z: BlobZone
    {
        T::deref_blob(blob)
    }
}

pub trait SavePtr<Z: BlobZone> : Sized {
    type Pending;
    type BlobPtr : 'static + Any;

    fn try_save<T: ?Sized + Save<Z>>(self, value: &T) -> Result<(Self, Self::BlobPtr), Self::Pending> {
        let mut state = value.init_save_state();

        value.save_poll(&mut state, self)
    }

    fn save<T: ?Sized + Save<Z>>(self, value: &T) -> (Self, Self::BlobPtr)
        where Self::Pending: Into<!>
    {
        match self.try_save(value) {
            Ok(r) => r,
            Err(never) => match never.into() {}
        }
    }

    fn save_blob(self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Self::BlobPtr),
                                                                         Self::Pending>;
}

pub trait LoadPtr<Z: BlobZone> {
    fn load_blob<'a, T: ?Sized + Load<Z>>(&self, ptr: &'a ValidPtr<T, Z::BlobPtr>) -> FullyValidBlob<'a, T, Z>;

    fn blob_zone(&self) -> &Z;
}

impl<Z: BlobZone, L: LoadPtr<Z>> LoadPtr<Z> for &'_ L {
    fn load_blob<'a, T: ?Sized + Load<Z>>(&self, ptr: &'a ValidPtr<T, Z::BlobPtr>) -> FullyValidBlob<'a, T, Z> {
        (*self).load_blob(ptr)
    }

    fn blob_zone(&self) -> &Z {
        (*self).blob_zone()
    }
}

impl LoadPtr<!> for () {
    fn load_blob<'a, T: ?Sized + Load<!>>(&self, ptr: &'a ValidPtr<T,!>) -> FullyValidBlob<'a, T, !> {
        match ptr.raw {}
    }

    fn blob_zone(&self) -> &! {
        panic!()
    }
}

pub trait ValidatePtr<Z: BlobZone> {
    type Error;

    fn validate_ptr<'p, T: ?Sized + Load<Z>>(&mut self, ptr: &'p FatPtr<T,Z::BlobPtr>)
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

impl SavePtr<!> for Vec<u8> {
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
