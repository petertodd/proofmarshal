//! Append-only, copy-on-write, persistence via byte slices and offsets.
//!
//! A `Pile` is a memory zone that (conceptually) consists of a linear byte slice. Pointers to data
//! within a pile are simply 64-bit, little-endian, integer `Offset`'s from the beginning of the
//! slice. The byte slice can come from either volatile memory (eg a `Vec<u8>`) or be a
//! memory-mapped file. `Offset` implements `Persist`, allowing types containing `Offset` pointers
//! to be memory-mapped.
//!
//! Mutation is provided by `PileMut` and `OffsetMut`, which extend `Offset` with copy-on-write
//! semantics: an `OffsetMut` is either a simple `Offset`, or a pointer to heap-allocated memory.
//! `OffsetMut` pointers also implement `Persist`, using the least-significant-bit to distinguish
//! between persistant offsets and heap memory pointers.

use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops;
use core::ptr::NonNull;
use core::pin::Pin;

use leint::Le;
use singlelife::Unique;

use super::*;

use crate::marshal::{*, blob::*};

use crate::coerce::{TryCastRef, CastRef};

pub mod offset;
use self::offset::Kind;
pub use self::offset::{Offset, OffsetMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pile<'pile, 'version> {
    marker: PhantomData<
        for<'a> fn(&'a Offset<'pile, 'version>) -> &'a [u8]
    >,
    slice: &'pile &'pile [u8],
}

impl Pile<'_, '_> {
    /// Creates a new `Pile` from a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::Pile;
    /// Pile::new([1,2,3,4], |pile| {
    /// })
    /// ```
    #[inline]
    pub fn new<R>(slice: impl AsRef<[u8]>, f: impl FnOnce(Pile) -> R) -> R {
        let slice = slice.as_ref();
        Unique::new(&slice, |slice| {
            f(Pile::from(slice))
        })
    }
}

impl<'p> From<Unique<'p, &&[u8]>> for Pile<'p, 'p> {
    #[inline]
    fn from(slice: Unique<'p, &&[u8]>) -> Pile<'p, 'p> {
        Self {
            marker: PhantomData,
            slice: Unique::into_inner(slice),
        }
    }
}

impl<'p> Pile<'p, 'static> {
    /// Creates an empty `Pile`.
    ///
    /// Note how the `'version` parameter is `'static': the earliest possible version of a pile is
    /// to have nothing in it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::{Pile, LoadTipError};
    /// let empty = Pile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert_eq!(empty.load_tip::<u8>().unwrap_err(),
    ///            LoadTipError::Undersized);
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.load_tip::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        Self {
            marker: PhantomData,
            slice: &EMPTY_SLICE,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoadTipError<E> {
    /// The pile is smaller than the size of the tip.
    Undersized,

    /// Validation of the tip itself failed.
    Tip(E),
}

impl<'p, 'v> Pile<'p, 'v> {
    /// Loads the tip of the `Pile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::Pile;
    /// Pile::new([0x12,0x34,0x45,0x78], |pile| {
    ///     let tip = pile.load_tip::<u32>().unwrap();
    ///     assert_eq!(*tip, 0x78453412);
    /// })
    /// ```
    pub fn load_tip<'a, T>(&'a self) -> Result<Ref<'a, T>, LoadTipError<T::Error>>
        where T: Decode<Offset<'p,'v>>
    {
        let start = self.slice.len().saturating_sub(T::BLOB_LAYOUT.size());
        let blob = &self.slice[start ..];
        let blob = Blob::<T, Offset<'p,'v>>::new(blob, ()).ok_or(LoadTipError::Undersized)?;

        let mut validator = T::validate_blob(blob).map_err(LoadTipError::Tip)?;

        match validator.poll(&mut self.clone()) {
            Ok(blob) => Ok(T::load_blob(blob, self)),
            Err(e) => todo!(),
        }
    }

    /// Creates a new version of an existing pile.
    ///
    /// The prefix of the new slice must match the existing pile; in debug builds a mismatch will
    /// panic.
    pub fn update<'v2>(&self, new_slice: Unique<'v2, &'p &[u8]>) -> Pile<'p, 'v2>
        where 'v: 'v2
    {
        assert!(self.slice.len() <= new_slice.len());
        debug_assert!(new_slice.starts_with(self.slice));

        Pile {
            marker: PhantomData,
            slice: Unique::into_inner(new_slice),
        }
    }

    #[inline]
    fn get_slice<'a>(&self, offset: &'a Offset<'p, 'v>, size: usize) -> Result<&'a [u8], offset::OffsetError> {
        let start = offset.get();

        // It's impossible for this to overflow as the maximum offset is just a quarter of
        // usize::MAX
        let end = start + size;
        self.slice.get(start .. end)
             .ok_or(offset::OffsetError { offset: start, size })
    }

    pub fn get_blob<'a, T>(&self, ptr: &'a FatPtr<T, Offset<'p, 'v>>) -> Result<Blob<'a, T, Offset<'p, 'v>>, offset::OffsetError>
        where T: ?Sized + Load<Offset<'p, 'v>>
    {
        self.get_slice(&ptr.raw, T::dyn_blob_layout(ptr.metadata).size())
                  .map(|slice| Blob::new(slice, ptr.metadata).unwrap())
    }
}

#[derive(Debug)]
pub enum ValidatePtrError {
    Offset(offset::OffsetError),
    Value(Box<dyn crate::marshal::Error>),
}

impl<'p,'v> ValidatePtr<Offset<'p, 'v>> for Pile<'p, 'v> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'p, 'v>>)
        -> Result<Option<BlobValidator<'a, T, Offset<'p,'v>>>, Self::Error>
    where T: ?Sized + Load<Offset<'p,'v>>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'p,'v> ValidatePtr<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&mut self, ptr: &'a FatPtr<T, Offset<'p,'v>>)
        -> Result<Option<BlobValidator<'a, T, OffsetMut<'p,'v>>>, Self::Error>
    where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob).map(Some)
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
    }
}

impl<'p,'v> Loader<Offset<'p,'v>> for Pile<'p,'v> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'p,'v>>)
        -> FullyValidBlob<'a, T, Offset<'p,'v>>
    where T: ?Sized + Load<Offset<'p,'v>>
    {
        self.get_blob(ptr)
            .map(|blob| unsafe { blob.assume_fully_valid() })
            .expect("ValidPtr to be valid")
    }
}

impl<'p,'v> Loader<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    fn load_blob<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'p,'v>>)
        -> FullyValidBlob<'a, T, OffsetMut<'p,'v>>
    where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        self.get_blob(ptr)
            .map(|blob| unsafe { blob.assume_fully_valid() })
            .expect("ValidPtr to be valid")
    }
}

impl AsRef<[u8]> for Pile<'_, '_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.slice[..]
    }
}

impl AsRef<[u8]> for PileMut<'_, '_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0.slice[..]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileMut<'s, 'v>(Pile<'s, 'v>);

impl<'p, 'v> PileMut<'p, 'v> {
    /// Creates a new version of an existing `PileMut`.
    ///
    /// Forwards to `Pile::update()`.
    pub fn update<'v2>(&self, new_slice: Unique<'v2, &'p &[u8]>) -> PileMut<'p, 'v2>
        where 'v: 'v2
    {
        self.0.update(new_slice).into()
    }

    /// Gets a blob from the pile.
    pub fn get_blob<'a, T>(&self, ptr: &'a FatPtr<T, Offset<'p, 'v>>) -> Result<Blob<'a, T, OffsetMut<'p,'v>>, offset::OffsetError>
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        self.get_slice(&ptr.raw, T::dyn_blob_layout(ptr.metadata).size())
                  .map(|slice| Blob::new(slice, ptr.metadata).unwrap())
    }

    /// Saves a value, producing a vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::PileMut;
    /// let pile = PileMut::default();
    ///
    /// assert_eq!(pile.save_to_vec(&42u8), &[42]);
    /// ```
    pub fn save_to_vec<T>(&self, value: &T) -> Vec<u8>
        where T: ?Sized + Save<OffsetMut<'p,'v>>
    {
        let mut state = value.init_save_state();
        let (dumper, offset) = value.save_poll(&mut state, VecDumper::default()).unwrap();
        dumper.0
    }
}

impl<'p,'v> From<Pile<'p,'v>> for PileMut<'p,'v> {
    #[inline]
    fn from(pile: Pile<'p,'v>) -> Self {
        Self(pile)
    }
}

impl<'p,'v> ops::Deref for PileMut<'p,'v> {
    type Target = Pile<'p,'v>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for PileMut<'_, 'static> {
    /// Returns an empty pile.
    #[inline]
    fn default() -> Self {
        PileMut::from(Pile::empty())
    }
}

impl<'p,'v> Alloc for PileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, OffsetMut<'p,'v>> {
        src.take_unsized(|src| unsafe {
            let metadata = T::metadata(src);
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(FatPtr { raw: OffsetMut::alloc::<T>(src), metadata }))
        })
    }

    #[inline]
    fn zone(&self) -> PileMut<'p,'v> {
        Self(self.0.clone())
    }
}

impl<'p,'v> Get<Offset<'p, 'v>> for Pile<'p, 'v> {
    fn get<'a, T>(&self, ptr: &'a ValidPtr<T, Offset<'p,'v>>) -> Ref<'a, T>
        where T: ?Sized + Load<Offset<'p,'v>>
    {
        let blob = self.load_blob(ptr);
        T::load_blob(blob, self)
    }

    fn take<T>(&self, ptr: OwnedPtr<T, Offset<'p,'v>>) -> T::Owned
        where T: ?Sized + Load<Offset<'p,'v>>
    {
        let blob = self.load_blob(&ptr);
        T::decode_blob(blob, self)
    }
}

impl<'p,'v> Get<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    fn get<'a, T>(&self, own: &'a ValidPtr<T, OffsetMut<'p,'v>>) -> Ref<'a, T>
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        match own.raw.kind() {
            Kind::Offset(_) => {
                let ptr: &'a ValidPtr<T, Offset<'p,'v>> = own.try_cast_ref().unwrap();
                let blob = self.load_blob(ptr);
                T::load_blob(blob, self)
            },
            Kind::Ptr(ptr) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(ptr.cast().as_ptr(), own.metadata)
                };
                Ref::Borrowed(r)
            },
        }
    }

    fn take<T>(&self, ptr: OwnedPtr<T, OffsetMut<'p,'v>>) -> T::Owned
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        let ptr = ptr.into_inner();
        if let Ok(valid_offset_ptr) = ptr.try_cast_ref() {
                let blob = self.load_blob(valid_offset_ptr);
                T::decode_blob(blob, self)
        } else {
            let FatPtr { raw, metadata } = ptr.into_inner();
            match unsafe { raw.try_take::<T>(metadata) } {
                Ok(owned) => owned,
                Err(_offset) => unreachable!(),
            }
        }
    }
}

impl<'p,'v> GetMut<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    fn get_mut<'a, T>(&self, ptr: &'a mut ValidPtr<T, OffsetMut<'p,'v>>) -> &'a mut T
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        let metadata = ptr.metadata;

        match ptr.raw.kind() {
            Kind::Offset(_) => {
                todo!()
            },
            Kind::Ptr(ptr) => {
                unsafe {
                    &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata)
                }
            },
        }
    }
}

pub fn save_to_vec<'p, 'v, T: ?Sized>(value: &T) -> Vec<u8>
where T: Save<OffsetMut<'p,'v>>
{
    let mut state = value.init_save_state();
    let (dumper, offset) = value.save_poll(&mut state, VecDumper::default()).unwrap();
    dumper.0
}

#[derive(Debug, Default)]
struct VecDumper(Vec<u8>);

impl<'p,'v> Dumper<OffsetMut<'p,'v>> for VecDumper {
    type Pending = !;

    fn try_save_ptr<'a, T: ?Sized + Pointee>(&self, ptr: &'a ValidPtr<T, OffsetMut<'p,'v>>) -> Result<Offset<'p,'v>, &'a T> {
        match ptr.raw.kind() {
            Kind::Offset(offset) => Ok(offset),
            Kind::Ptr(nonnull) => {
                let r: &'a T = unsafe {
                    &*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata)
                };
                Err(r)
            }
        }
    }

    fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'p,'v>), !> {
        let offset = self.0.len();

        self.0.resize(offset + size, 0);
        f(&mut self.0[offset ..]);

        Ok((self, Offset::new(offset).unwrap()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use singlelife::unique;

    #[test]
    #[should_panic(expected = "new_slice.starts_with(self.slice)")]
    fn pile_update_panics_on_mismatch() {
        Pile::new([1,2,3], |pile|
            Unique::new(&&[2,2,3][..], |slice2| {
                pile.update(slice2);

                // make the tests pass in release builds
                assert!(cfg!(debug_assertions), "new_slice.starts_with(self.slice)");
            })
        )
    }

    #[test]
    #[should_panic(expected = "self.slice.len() <= new_slice.len()")]
    fn pile_update_panics_on_shorter() {
        Pile::new([1], |pile|
            Unique::new(&&[][..], |slice2| {
                pile.update(slice2);
            })
        )
    }

/*
    #[derive(Debug, Default)]
    struct DeepDump {
        dst: Vec<u8>,
    }

    impl<'s,'m> Dumper<Offset<'s,'m>> for DeepDump {
        type Pending = !;

        fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, Offset<'s,'m>>)
            -> Result<Offset<'s,'m>, &'p T>
        {
            Ok(ptr.raw)
        }

        fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s,'m>), !> {
            let offset = self.dst.len();
            self.dst.resize(offset + size, 0);

            f(&mut self.dst[offset..]);

            let offset = Offset::new(offset).unwrap();
            Ok((self, offset))
        }
    }

    #[derive(Debug, Default)]
    struct DeepDumpMut {
        dst: Vec<u8>,
    }

    impl<'s,'m> Dumper<OffsetMut<'s,'m>> for DeepDumpMut {
        type Pending = !;

        fn try_save_ptr<'p, T: ?Sized + Pointee>(&self, ptr: &'p ValidPtr<T, OffsetMut<'s,'m>>)
            -> Result<Offset<'s,'m>, &'p T>
        {
            match ptr.raw.kind() {
                Kind::Offset(offset) => Ok(offset),
                Kind::Ptr(nonnull) => {
                    let r: &'p T = unsafe {
                        &*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata)
                    };
                    Err(r)
                },
            }
        }

        fn try_save_blob(mut self, size: usize, f: impl FnOnce(&mut [u8])) -> Result<(Self, Offset<'s,'m>), !> {
            let offset = self.dst.len();
            self.dst.resize(offset + size, 0);

            f(&mut self.dst[offset..]);

            let offset = Offset::new(offset).unwrap();
            Ok((self, offset))
        }
    }

    #[test]
    fn deepdump_pile() {
        let slice = &vec![0x78,0x56,0x34,0x12][..];
        unique!(|&slice| {
            let pile = Pile::new(slice);

            let offset = Offset::new(0).unwrap();
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            let dumper = DeepDump::default();

            let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
            assert_eq!(dumper.dst, &[]);
        })
    }

    #[test]
    fn deepdump_pilemut() {
        Unique::new(&[0x78,0x56,0x34,0x12][..], |pile| {
            let mut pile = PileMut::from(Pile::new(&pile));

            let offset = OffsetMut::from(Offset::new(0).unwrap());
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned_ptr = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            let dumper = DeepDumpMut::default();

            let offset = dumper.try_save_ptr(&owned_ptr).unwrap();
            assert_eq!(dumper.dst, &[]);

            let owned_ptr = pile.alloc(0xabcd_u16);
            let r = dumper.try_save_ptr(&owned_ptr).unwrap_err();
            assert_eq!(r, &0xabcd);
            assert_eq!(dumper.dst, &[]);
        })
    }
*/

    #[test]
    fn pilemut_get() {
        /*
        Unique::new(&&[0x78,0x56,0x34,0x12][..], |pile| {
            let mut static_pile = PileMut::default();
            let static_owned = static_pile.alloc(12u8);

            let pile = PileMut::from(Pile::new(pile));

            let offset = Offset::new(0).unwrap();
            let offset = OffsetMut::from(offset);
            let fatptr: FatPtr<u32,_> = FatPtr { raw: offset, metadata: () };
            let owned = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };

            assert_eq!(*pile.get(&owned), 0x12345678);

            assert_eq!(*pile.get(&static_owned), 12);
            assert_eq!(*static_pile.get(&static_owned), 12);

            /*
            let buf = &[0x78,0x56,0x34,0x12,0x42][..];
            (||{
                let pile2 = unsafe { pile.update(&buf) };
                let pile2 = PileMut::from(pile2);

                assert_eq!(*pile2.get(&static_owned), 12);
                assert_eq!(*pile2.get(&owned), 0x12345678);

                let offset = pile2.new_offset(4).unwrap();
                let offset = OffsetMut::from(offset);
                let fatptr: FatPtr<u8,_> = FatPtr { raw: offset, metadata: () };
                let owned2 = unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(fatptr)) };
                assert_eq!(*pile2.get(&owned2), 0x42);

                // doesn't compile as pile is out of date
                //assert_eq!(*pile.get(&owned2), 0x42);
            })()
            */
        })
    */
    }

    #[test]
    fn pilemut_getmut() {
        let mut zone = PileMut::default();
        let mut owned = zone.alloc((1,2,3));

        let r = zone.get_mut(&mut owned);

        assert_eq!(*r, (1,2,3));
        r.0 = 10;
        assert_eq!(*r, (10,2,3));

        let r = zone.get_mut(&mut owned);
        assert_eq!(*r, (10,2,3));
    }

    #[test]
    fn offsetmut_clone() {
        let mut zone = PileMut::default();
        let v1 = zone.alloc((1,2,3));
        let v2 = v1.clone();
        assert_ne!(v1.raw, v2.raw);
    }
}
