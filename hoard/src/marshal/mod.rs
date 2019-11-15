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

pub fn decode<T: Decode<()>>(blob: &[u8]) -> Result<T, T::Error> {
    let blob = Blob::new(blob, T::make_sized_metadata()).expect("wrong size");

    let mut validator = T::validate_blob(blob)?;
    let fully_valid_blob = validator.poll(&mut ()).unwrap();
    Ok(T::decode_blob(fully_valid_blob, &()))
}

pub unsafe trait Save<Q> : Pointee + Owned {
    fn blob_layout(metadata: Self::Metadata) -> BlobLayout;

    type State;
    fn init_save_state(&self) -> Self::State;

    fn save_poll<D: Dumper<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending>;
}

pub trait Load<P> : Save<P> {
    type Error;

    type ValidateChildren : ValidateChildren<P>;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
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


/*
impl<P, T> Decode<P> for T
where T: Primitive
{
    type Error = T::Error;


    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl Loader<P>) -> Self {
        todo!()
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
        todo!()
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        todo!()
    }
}
*/

pub trait Encode<Q> : Sized {
    const BLOB_LAYOUT: BlobLayout;

    type State;
    fn init_encode_state(&self) -> Self::State;

    fn encode_poll<D: Dumper<Q>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending>;
    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error>;

    fn encode_own<T: ?Sized + Pointee>(own: &Own<T,Self>) -> Result<Q::State, <T as Save<Q>>::State>
        where T: Save<Q>,
              Q: Encode<Q>,
              Self: Ptr
    {
        unimplemented!()
    }

    fn encode_own_value<T, D>(own: &Own<T,Self>, state: &mut T::State, dumper: D) -> Result<(D, Q::State), D::Pending>
        where T: ?Sized + Save<Q>,
              D: Dumper<Q>,
              Q: Encode<Q>,
              Self: Ptr
    {
        unimplemented!()
    }

    fn encode_own_ptr<W: WriteBlob>(&self, ptr_state: &Q::State, dst: W) -> Result<W::Ok, W::Error>
        where Q: Encode<Q>,
              Self: Ptr,
    {
        unimplemented!()
    }
}

pub trait Decode<P> : Encode<P> {
    type Error;

    type ValidateChildren : ValidateChildren<P>;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, P>) -> &'p Self
        where Self: Persist
    {
        todo!()
    }
}

impl<Q, T> Encode<Q> for T
where T: Primitive
{
    const BLOB_LAYOUT: BlobLayout = T::BLOB_LAYOUT;

    type State = ();
    fn init_encode_state(&self) -> () { }

    fn encode_poll<D: Dumper<Q>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending> {
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

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl Loader<P>) -> Self {
        T::decode_blob(blob)
    }

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, P>, _: &impl Loader<P>) -> Ref<'p, Self> {
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

    fn save_poll<D: Dumper<P>>(&self, state: &mut Self::State, dumper: D) -> Result<(D, D::BlobPtr), D::Pending> {
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

pub trait Dumper<Q> : Sized {
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

pub trait Loader<P> {
}

impl<P, T: Loader<P>> Loader<P> for &'_ T {
}

impl Loader<()> for () {
}
impl Loader<!> for () {
}

pub trait ValidatePtr<P> {
    type Error;

    //fn validate_ptr<T: ?Sized + Load<P>>(&mut self, metadata: T::Metadata)
}

impl ValidatePtr<!> for () {
    type Error = !;
}
impl ValidatePtr<()> for () {
    type Error = !;
}

impl Dumper<()> for Vec<u8> {
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

/*

impl<P, T: Decode<P>> Load<P> for T {
    type Error = T::Error;

    type ValidateChildren = T::ValidateChildren;
    fn validate_blob<'p>(blob: Blob<'p, Self, P>) -> Result<BlobValidator<'p, Self, P>, Self::Error> {
        T::validate_blob(blob)
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, P>, loader: &impl Loader<P>) -> Self {
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

/*

pub trait SavePoll<P> : Sized {
    type Target : ?Sized + Pointee<Metadata = Self::TargetMetadata>;
    type TargetMetadata : Persist + Primitive + Copy + fmt::Debug + Eq + Ord + core::hash::Hash + Send + Sync;

    fn poll<D: Dumper<P>>(&mut self, dumper: D) -> Result<(D, D::PtrEncoder, Self::TargetMetadata), D::Pending>;
}


pub struct ValueSaver<T: Encode<P>, P> {
    encoder: T::EncodePoll,
}

impl<P, T: Encode<P>> SavePoll<P> for ValueSaver<T, P> {
    type Target  = T;
    type TargetMetadata = ();

    fn poll<D: Dumper<P>>(&mut self, dumper: D) -> Result<(D, D::PtrEncoder, ()), D::Pending> {
        let dumper = self.encoder.poll(dumper)?;
        todo!()
    }
}




*/







/*
/// A *value* that can be saved in a zone.
pub trait Save<Z: Zone> : Owned + Pointee {

    #[inline(always)]
    fn blob_layout(metadata: Self::Metadata) -> BlobLayout {
        assert_eq!(mem::size_of_val(&metadata), 0);

        Self::BLOB_LAYOUT
    }

    type SavePoll : SavePoll<Z, Target = Self>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll;

    /// Hook to allow zones to define how to save pointers.
    fn save_ptr<T, S>(ptr: Own<T, Self>, saver: &mut S) -> Result<Z::PersistPtr, T::SavePoll>
        where T: ?Sized + Save<Z>,
              S: SavePtr<Z>,
              Self: Zone,
    {
        unimplemented!()
    }
}

pub trait SavePoll<Z: Zone> : Sized {
    type Target : ?Sized + Save<Z>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Z>
    {
        let _ = ptr_saver;
        Poll::Ready(Ok(()))
    }


    fn metadata(&self) -> <Self::Target as Pointee>::Metadata {
        if mem::size_of::<<Self::Target as Pointee>::Metadata>() == 0 {
            unsafe { MaybeUninit::uninit().assume_init() }
        } else {
            unimplemented!()
        }
    }
}

pub trait SavePtr<Z: Zone> {
    type Error;

    fn save_blob(&mut self, size: usize, f: impl FnOnce(&mut [u8]))
        -> Result<Z::PersistPtr, Self::Error>;

    fn save_own<T: ?Sized + Save<Z>>(&mut self, own: Own<T, Z>)
        -> Result<Z::PersistPtr, T::SavePoll>;
}

impl SavePtr<!> for () {
    type Error = !;

    fn save_blob(&mut self, _: usize, _: impl FnOnce(&mut [u8]))
        -> Result<!, Self::Error>
    {
        panic!()
    }

    fn save_own<T: ?Sized + Save<!>>(&mut self, own: Own<T, !>)
        -> Result<!, T::SavePoll>
    {
        match *own.ptr() {}
    }
}


pub trait Load<Z: Zone> : Save<Z> {
    type Error;

    type ValidateChildren : ValidateChildren<Z>;
    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error>;

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self::Owned;

    fn load_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Ref<'p, Self> {
        Ref::Owned(Self::decode_blob(blob, loader))
    }

    fn deref_blob<'p>(blob: FullyValidBlob<'p, Self, Z>) -> &'p Self
        where Self: Persist
    {
        todo!()
    }
}


pub trait Loader<Z: Zone> {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: Z::PersistPtr, metadata: T::Metadata) -> Own<T,Z>;

    fn zone(&self) -> Z;
    fn allocator(&self) -> Z::Allocator;
}

impl Loader<!> for () {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: !, _: T::Metadata) -> Own<T,!> {
        match persist_ptr {}
    }

    fn zone(&self) -> ! {
        panic!()
    }

    fn allocator(&self) -> crate::never::NeverAllocator<!> {
        panic!()
    }
}

impl<Z: Zone, L: Loader<Z>> Loader<Z> for &'_ L {
    fn load_ptr<T: ?Sized + Pointee>(&self, persist_ptr: Z::PersistPtr, metadata: T::Metadata) -> Own<T,Z> {
        (&**self).load_ptr(persist_ptr, metadata)
    }

    fn zone(&self) -> Z {
        (&**self).zone()
    }

    fn allocator(&self) -> Z::Allocator {
        (&**self).allocator()
    }
}


impl ValidatePtr<!> for () {
    type Error = !;
}

impl<'a, Z: Zone, T: ValidatePtr<Z>> ValidatePtr<Z> for &'a mut T {
    type Error = T::Error;
}

*/


#[derive(Debug)]
pub enum SaveOwnPoll<T: ?Sized + Save<Q>, P: Ptr, Q> {
    Own(Own<T,P>),
    //Pending(T::SavePoll),
    Done {
        //persist_ptr: Y::PersistPtr,
        ptr: Q,
        metadata: T::Metadata,
    },
    Poisoned,
}

/*
impl<T: ?Sized, Z: Zone, Y: Zone> Save<Y> for Own<T,Z>
where T: Save<Y>,
      Z: Save<Y>,
{
    const BLOB_LAYOUT: BlobLayout = <Y::PersistPtr as Save<!>>::BLOB_LAYOUT
                                        .extend(<T::Metadata as Save<!>>::BLOB_LAYOUT);

    type SavePoll = SaveOwnPoll<T, Z, Y>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        SaveOwnPoll::Own(this.take_sized())
    }
}

impl<T: ?Sized, Z: Zone, Y: Zone> SavePoll<Y> for SaveOwnPoll<T, Z, Y>
where T: Save<Y>,
      Z: Save<Y>,
{
    type Target = Own<T,Z>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Y>
    {
        match self {
            Self::Done { .. } => Ok(()).into(),
            Self::Poisoned => panic!(),

            Self::Pending(pending) =>
                match pending.save_children(ptr_saver)? {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(()) => {
                        let metadata = pending.metadata();
                        let size = T::blob_layout(metadata).size();
                        let persist_ptr = ptr_saver.save_blob(size, |dst| {
                            pending.encode_blob(dst).unwrap()
                        })?;

                        *self = Self::Done { persist_ptr, metadata };

                        Ok(()).into()
                    },
                },
            Self::Own(_) => {
                if let Self::Own(own) = mem::replace(self, Self::Poisoned) {
                    let metadata = own.metadata();
                    match Z::save_ptr(own, ptr_saver) {
                        Ok(persist_ptr) => {
                            *self = Self::Done { persist_ptr, metadata };
                            Ok(()).into()
                        },
                        Err(pending) => {
                            *self = Self::Pending(pending);
                            self.save_children(ptr_saver)
                        },
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        if let Self::Done { persist_ptr, metadata } = self {
            unsafe fn as_bytes<T>(x: &T) -> &[u8] {
                slice::from_raw_parts(x as *const T as *const u8,
                                      mem::size_of::<T>())
            }

            unsafe {
                // FIXME
                dst.write_bytes(as_bytes(persist_ptr))?
                   .write_bytes(as_bytes(metadata))?
                   .done()
            }
        } else {
            panic!()
        }
    }

}

pub enum ValidateOwnError<T: ?Sized + Pointee, Z: Zone> {
    Ptr(<Z::PersistPtr as Load<!>>::Error),
    Metadata(<T::Metadata as Load<!>>::Error),
}

pub enum ValidateOwn<T: ?Sized + Load<Z>, Z: Zone> {
    Own {
        ptr: Z::PersistPtr,
        metadata: T::Metadata,
    },
    Value(T::ValidateChildren),
}

impl<T: ?Sized + Pointee, Z: Zone> Load<Z> for Own<T,Z>
where T: Load<Z>,
      Z: Load<Z>,
{
    type Error = ValidateOwnError<T, Z>;

    type ValidateChildren = ValidateOwn<T,Z>;

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        // FIXME: validate metadata properly

        let primitive_blob = Blob::<(Z::PersistPtr, T::Metadata), !>::try_from(&blob[..]).unwrap();

        let mut v = primitive_blob.validate();
        let _ = v.field::<Z::PersistPtr>().map_err(|e| ValidateOwnError::Ptr(e))?;
        let _ = v.field::<T::Metadata>().map_err(|e| ValidateOwnError::Metadata(e))?;

        let (ptr, metadata) = *try_decode(primitive_blob).unwrap();

        Ok(blob.assume_valid(ValidateOwn::Own { ptr, metadata }))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Self {
        let mut fields = blob.decode_struct(loader);
        let ptr = fields.field::<Z::PersistPtr>();

        let primitive_blob = Blob::<(Z::PersistPtr, T::Metadata), !>::try_from(&blob[..]).unwrap();
        let (ptr, metadata) = *try_decode(primitive_blob).unwrap();

        loader.load_ptr(ptr, metadata)
    }
}

impl<T: ?Sized + Load<Z>, Z: Zone> ValidateChildren<Z> for ValidateOwn<T,Z> {
    fn validate_children<V>(&mut self, validator: &mut V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        match self {
            ValidateOwn::Own { ptr, metadata } => {
                todo!()
            },
            ValidateOwn::Value(v) => v.validate_children(validator),
        }
    }
}

pub fn encode<T: Save<!>>(value: T) -> Vec<u8> {
    let mut dst = vec![0; T::BLOB_LAYOUT.size()];
    encode_into(value, &mut dst[..]);
    dst
}

pub fn encode_into<T: Save<!>>(value: T, dst: &mut [u8]) {
    let mut saver = T::save_poll(value);

    match saver.save_children(&mut ()) {
        Poll::Ready(Ok(())) => {},
        _ => panic!(),
    }

    saver.encode_blob(&mut dst[..]).unwrap();
}

pub fn try_decode<'a, T: ?Sized + Load<!>>(blob: Blob<'a, T,!>) -> Result<Ref<'a, T>, T::Error> {
    let mut validator = T::validate_blob(blob)?;

    match validator.poll(&mut ()) {
        Poll::Ready(Ok(fully_valid_blob)) => Ok(T::load_blob(fully_valid_blob, &mut ())),
        _ => panic!(),
    }
}

pub fn test_try_decode(blob: Blob<(u8,Option<(u8, Option<u16>)>), !>) -> Result<Ref<(u8, Option<(u8, Option<u16>)>)>, impls::TupleError> {
    try_decode(blob)
}

pub fn test_encode(v: (u8, Option<(u8, Option<u16>)>), dst: &mut [u8;6]) {
    encode_into(v, dst)
}

#[cfg(test)]
mod test {
    use super::*;

    use core::convert::TryFrom;

    #[test]
    fn test() {
        let blob = Blob::<(u8, Option<(u8, Option<u8>)>),!>::try_from(&[0;5][..]).unwrap();

        let _validator = <(u8, Option<(u8, Option<u8>)>)>::validate_blob(blob);
    }
}
*/
*/
