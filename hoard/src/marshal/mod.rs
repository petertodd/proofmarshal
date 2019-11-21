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

pub fn encode<T: ?Sized + Save<()>>(value: &T) -> Vec<u8> {
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

pub unsafe trait Save<Q> : Pointee + Owned {
    fn blob_layout(metadata: Self::Metadata) -> BlobLayout;

    type State;
    fn init_save_state(&self) -> Self::State;

    fn save_poll<D: SavePtr<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending>;
}

pub trait Error : 'static + Any + fmt::Debug + Send {
    fn type_name(&self) -> &'static str;
}

impl<E: ?Sized + 'static + Any + fmt::Debug + Send> Error for E {
    fn type_name(&self) -> &'static str {
        core::any::type_name::<E>()
    }
}

pub trait Load<P> : Save<P> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<P>;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl LoadPtr<P>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl LoadPtr<P>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        todo!()
    }
}

pub trait ValidateChildren<P> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>;
}

impl<P> ValidateChildren<P> for () {
    fn validate_children<V>(&mut self, _: &mut V) -> Result<(), V::Error>
        where V: ValidatePtr<P>
    {
        Ok(())
    }
}

pub trait Encode<Q> : Sized {
    const BLOB_LAYOUT: BlobLayout;

    type State;
    fn init_encode_state(&self) -> Self::State;

    fn encode_poll<D: SavePtr<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>;
    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;

    fn encode_own<T: ?Sized + Pointee>(own: &Own<T,Self>) -> Result<Self::State, <T as Save<Q>>::State>
        where T: Save<Q>,
              Q: Encode<Q>,
              Self: Ptr
    {
        unimplemented!()
    }

    fn encode_own_value<T, D>(own: &Own<T,Self>, state: &mut T::State, dumper: D) -> Result<(D, Self::State), D::Pending>
        where T: ?Sized + Save<Q>,
              D: SavePtr<Q>,
              Q: Encode<Q>,
              Self: Ptr
    {
        unimplemented!()
    }

    /*
    fn encode_own_ptr<W: WriteBlob>(&self, ptr_state: &Self::State, dst: W) -> Result<W::Ok, W::Error>
        where Q: Encode<Q>,
              Self: Ptr,
    {
        unimplemented!()
    }
    */
}

pub trait Decode<Q> : Encode<Q> {
    type Error : Error;

    type ValidateChildren : ValidateChildren<Q>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Q>) -> Result<BlobValidator<'p, Self, Q>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Q>, loader: &impl LoadPtr<Q>) -> Self;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Q>, loader: &impl LoadPtr<Q>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Q>) -> &'p Self
        where Self: Persist
    {
        todo!()
    }

    fn ptr_validate_blob<'a>(blob: Blob<'a, Self, Q>) -> Result<FullyValidBlob<'a, Self, Q>, Self::Error>
        where Self: Ptr
    {
        unimplemented!()
    }

    fn ptr_decode_blob<'a>(blob: FullyValidBlob<'a, Self, Q>) -> Self
        where Self: Ptr
    {
        unimplemented!()
    }

    fn ptr_validate_children<'p, T, V>(ptr: &'p FatPtr<T,Self>, validator: &mut V)
            -> Result<Option<BlobValidator<'p, T, Q>>, V::Error>
        where Self: Ptr,
              T: ?Sized + Load<Q>,
              V: ValidatePtr<Q>,
    {
        unimplemented!()
    }
}

impl<Q, T> Encode<Q> for T
where T: Primitive
{
    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT;

    type State = ();
    fn init_encode_state(&self) -> () { }

    fn encode_poll<D: SavePtr<Q>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(dst)
    }
}

impl<P, T> Decode<P> for T
where T: Primitive
{
    type Error = T::Error;

    type ValidateChildren = ();
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        T::validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl LoadPtr<P>) -> Self {
        T::decode_blob(blob)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl LoadPtr<P>) -> Ref<'p, Self> {
        Self::load_blob(blob)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        Self::deref_blob(blob)
    }
}

unsafe impl<P, T: Encode<P>> Save<P> for T {
    #[inline(always)]
    fn blob_layout(_: ()) -> BlobLayout {
        T::BLOB_LAYOUT
    }

    type State = T::State;
    fn init_save_state(&self) -> Self::State {
        self.init_encode_state()
    }

    fn save_poll<D: SavePtr<P>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending> {
        let dumper = self.encode_poll(state, dumper)?;
        dumper.save_blob(Self::blob_layout(()).size(), | dst | {
            match self.encode_blob(state, dst) {
                Ok(()) => (),
                Err(never) => never,
            }
        })
    }
}

impl<P, T: Decode<P>> Load<P> for T {
    type Error = T::Error;

    type ValidateChildren = T::ValidateChildren;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        T::validate_blob(blob)
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl LoadPtr<P>) -> Self::Owned {
        T::decode_blob(blob, loader)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl LoadPtr<P>) -> Ref<'p, Self> {
        T::load_blob(blob, loader)
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        T::deref_blob(blob)
    }
}

pub trait SavePtr<Q> : Sized {
    type Pending;
    type BlobPtr : 'static + Any;

    fn try_save<T: ?Sized + Save<Q>>(self, value: &T) -> Result<(Self, Self::BlobPtr), Self::Pending> {
        let mut state = value.init_save_state();

        value.save_poll(&mut state, self)
    }

    fn save<T: ?Sized + Save<Q>>(self, value: &T) -> (Self, Self::BlobPtr)
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

pub trait LoadPtr<P> {
    fn load_blob<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T,P>) -> FullyValidBlob<'a, T, P>;
}

impl<P, L: LoadPtr<P>> LoadPtr<P> for &'_ L {
    fn load_blob<'a, T: ?Sized + Load<P>>(&self, ptr: &'a ValidPtr<T,P>) -> FullyValidBlob<'a, T, P> {
        (*self).load_blob(ptr)
    }
}

impl LoadPtr<!> for () {
    fn load_blob<'a, T: ?Sized + Load<!>>(&self, ptr: &'a ValidPtr<T,!>) -> FullyValidBlob<'a, T, !> {
        match ptr.raw {}
    }
}

pub trait ValidatePtr<Q> {
    type Error;

    fn validate_ptr<'p, T: ?Sized + Load<Q>>(&mut self, ptr: &'p FatPtr<T,Q>)
        -> Result<Option<BlobValidator<'p, T, Q>>, Self::Error>;
}

impl ValidatePtr<!> for () {
    type Error = !;

    fn validate_ptr<'p, T: ?Sized + Load<!>>(&mut self, ptr: &'p FatPtr<T,!>)
        -> Result<Option<BlobValidator<'p, T, !>>, !>
    {
        match ptr.raw {}
    }
}

impl SavePtr<()> for Vec<u8> {
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
