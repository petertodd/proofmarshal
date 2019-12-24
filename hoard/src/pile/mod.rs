//! Persistence via "piles" of copy-on-write append-only bytes.
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

use core::cmp;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;
use core::ptr::{self, NonNull};

use std::alloc::Layout;

use owned::Take;
use singlelife::Unique;

use crate::{
    pointee::Pointee,
    zone::{
        Alloc,
        Get, TryGet,
        GetMut, TryGetMut,
        ValidPtr, OwnedPtr, FatPtr,
        Zone,
        PtrError, PtrResult,
        refs::{Own, Ref, RefMut},
        never::NeverAllocator,
    },

    blob::*,
    load::*,
    save::*,
};

pub mod offset;
use self::offset::{Offset, OffsetMut};

pub mod error;
use self::error::*;

/*
pub mod mapping;
use self::mapping::Mapping;
*/

/// Fallible, unverified, `Pile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pile<'pile, 'version> {
    marker: PhantomData<
        for<'a> fn(&'a Offset<'pile, 'version>) -> &'a [u8]
    >,
    //mapping: &'pile dyn Mapping,
    mapping: &'pile &'pile [u8],
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
            mapping: &*Unique::into_inner(slice),
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
    /// ```no_compile
    /// # use hoard::pile::{Pile, FullyValidateError};
    /// let empty = Pile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert_eq!(empty.fully_validate::<u8>().unwrap_err(),
    ///            FullyValidateError::Undersized);
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.fully_validate::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        Self {
            marker: PhantomData,
            mapping: &EMPTY_SLICE,
        }
    }
}

impl<'p, 'v> Pile<'p, 'v> {
    fn slice(&self) -> &'p &'p [u8] {
        unsafe {
            //&*(self.mapping as *const dyn Mapping as *const &'p [u8])
            &*(self.mapping as *const _ as *const &'p [u8])
        }
    }

    /*
    /// Loads the tip of the `Pile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::Pile;
    /// Pile::new([12], |pile| {
    ///     let tip = pile.fully_validate::<u8>().unwrap();
    ///     assert_eq!(**tip, 12);
    /// })
    /// ```
    pub fn fully_validate<T: Load<Self>>(&self) -> Result<Ref<'p, T, Self>, FullyValidateError<T::Error>>
    {
        let blob = self.get_tip_blob().ok_or(FullyValidateError::Undersized)?;
        let blob = T::validate(blob.into_validator()).map_err(FullyValidateError::Tip)?;

        unsafe {
            let r = blob.to_ref();
            let poll = r.validate_children();

            /* poll.poll(self).map_err(FullyValidateError::Child)?; */

            Ok(Ref {
                this: r,
                zone: *self,
            })
        }
    }

    pub fn get_tip_blob<T>(&self) -> Option<Blob<'p, T>> {
        let start = self.slice().len().saturating_sub(mem::size_of::<T>());
        let blob = &self.slice()[start ..];
        Blob::<T>::new(blob, ())
    }

    /// Creates a new version of an existing pile.
    ///
    /// The prefix of the new slice must match the existing pile; in debug builds a mismatch will
    /// panic.
    pub fn update<'v2>(&self, new_slice: Unique<'v2, &'p &[u8]>) -> Pile<'p, 'v2>
        where 'v: 'v2
    {
        assert!(self.slice().len() <= new_slice.len());
        debug_assert!(new_slice.starts_with(self.slice()));

        Pile {
            marker: PhantomData,
            mapping: &*Unique::into_inner(new_slice),
        }
    }

    pub fn get_blob<'a, T>(&self, ptr: &'a FatPtr<T, Self>) -> Result<Blob<'a, T>, OffsetError<'p,'v>>
        where T: ?Sized + Validate
    {
        let offset = ptr.raw;
        let start = offset.get();
        let size = T::layout(ptr.metadata).size();

        // It's impossible for this to overflow as the maximum offset is just a quarter of
        // usize::MAX
        let end = start + size;
        if let Some(slice) = self.slice().get(start .. end) {
            Ok(Blob::new(slice, ptr.metadata).unwrap())
        } else {
            Err(OffsetError::new(self, ptr))
        }
    }
    */
}


impl<'p,'v> Zone for Pile<'p,'v> {
    type Ptr = Offset<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;
    type Allocator = NeverAllocator<Self>;

    type Error = OffsetError<'p,'v>;

    #[inline(always)]
    fn duplicate(&self) -> Self {
        *self
    }

    fn clone_ptr<T>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        todo!()
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        //Err(FatPtr::clone(ptr))
        todo!()
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let fatptr = owned.into_inner().into_inner();
        todo!() //f(Err(fatptr))
    }
}

/// Mutable, unverified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileMut<'s, 'v>(Pile<'s, 'v>);

impl<'p,'v> Zone for PileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;
    type Allocator = Self;

    type Error = OffsetError<'p,'v>;

    #[inline(always)]
    fn allocator() -> Self
        where Self: Default
    {
        Self::default()
    }

    #[inline(always)]
    fn duplicate(&self) -> Self {
        self.clone()
    }

    fn clone_ptr<T>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        todo!()
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        /*
        match ptr.raw.kind() {
            offset::Kind::Ptr(nonnull) => unsafe {
                Ok(&*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata))
            },
            offset::Kind::Offset(raw) => Err(FatPtr { raw, metadata: ptr.metadata }),
        }*/ todo!()
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        ptr: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        /*
        let FatPtr { raw, metadata } = ptr.into_inner().into_inner();
        match raw.kind() {
            offset::Kind::Ptr(nonnull) => unsafe {
                let v: &mut T = &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), metadata);
                let v: &mut ManuallyDrop<T> = &mut *(v as *mut _ as *mut _);

                let layout = Self::min_align_layout(Layout::for_value(v));
                let r = f(Ok(v));

                if layout.size() > 0 {
                    std::alloc::dealloc(v as *mut _ as *mut u8, layout)
                }
                r
            },
            offset::Kind::Offset(offset) => {
                f(Err(FatPtr { raw: offset, metadata }))
            }
        }*/ todo!()
    }
}

/*
impl<Z> Encoded<Z> for Pile<'_, '_> {
    type Encoded = ();
}

impl<'a, 'p, 'v> Encode<'a, Self> for Pile<'p, 'v> {
    type State = ();

    fn save_children(&self) -> () {}

    fn poll<D: Dumper<Pile<'p,'v>>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.finish()
    }

    fn zone_save_ptr<T, D>(ptr: &'a ValidPtr<T, Self>, dumper: &D)
        -> Result<<Self as Zone>::PersistPtr, &'a T>
        where T: ?Sized + Pointee,
              D: Dumper<Self>
    {
        /*
        let ptr: ValidPtr<T, Pile<'p, 'v2>> = unsafe { mem::transmute_copy(ptr) };
        match dumper.save_ptr(&ptr) {
            Ok(persist) => Ok(persist),
            Err(r) => todo!(),
        }*/ todo!()
    }
}

impl<Z> Encoded<Z> for PileMut<'_, '_> {
    type Encoded = ();
}

impl<'a, 'p, 'v, 'p2, 'v2> Encode<'a, Pile<'p2, 'v2>> for PileMut<'p, 'v> {
    type State = ();

    fn save_children(&self) -> () {}

    fn poll<D: Dumper<Pile<'p2,'v2>>>(&self, _: &mut (), dumper: D) -> Result<D, D::Error> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.finish()
    }

    fn zone_save_ptr<T, D>(ptr: &'a ValidPtr<T, Self>, dumper: &D) -> Result<Offset<'static, 'static>, &'a T>
        where T: ?Sized + Pointee,
              D: Dumper<Pile<'p2,'v2>>
    {
        match Self::try_get_dirty(ptr) {
            Ok(r) => Err(r),
            Err(persist_ptr) => {
                todo!()
            },
        }
    }
}

impl<'p, 'v> TryGet for Pile<'p, 'v> {
    fn try_get<'a, T: ?Sized + Validate>(&self, ptr: &'a ValidPtr<T, Self>) -> PtrResult<Ref<'a, T, Self>, T, Self> {
        let blob = self.get_blob(ptr).map_err(|e| PtrError::Ptr(e))?;

        match T::validate(blob.into_validator()) {
            Ok(valid_blob) => {
                Ok(Ref {
                    this: unsafe { valid_blob.to_ref() },
                    zone: *self,
                })
            },
            Err(blob::Error::Value(e)) => Err(PtrError::Value(e)),
            Err(blob::Error::Padding) => {
                todo!()
            },
        }
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> PtrResult<Own<T::Owned, Self>, T, Self> {
        let r = self.try_get(&ptr)?;

        unsafe {
            let r: &ManuallyDrop<T> = &*(r.this as *const _ as *const _);

            Ok(Own {
                this: T::to_owned(r),
                zone: *self,
            })
        }
    }
}

impl<'p, 'v> TryGet for PileMut<'p, 'v> {
    fn try_get<'a, T: ?Sized + Validate>(&self, ptr: &'a ValidPtr<T, Self>) -> PtrResult<Ref<'a, T, Self>, T, Self> {
        /*
        match Self::try_get_dirty(ptr) {
            Ok(r) => Ok(Ref { this: r, zone: *self }),
            Err(_fatptr) => {
                let ptr = unsafe { &*(ptr as *const _ as *const ValidPtr<T, Self::Persist>) };
                let blob = self.get_blob(ptr).map_err(|e| PtrError::Ptr(e))?;

                match T::validate(blob.into_validator()) {
                    Ok(valid_blob) => {
                        Ok(Ref {
                            this: unsafe { valid_blob.to_ref() },
                            zone: *self,
                        })
                    },
                    Err(blob::Error::Value(e)) => Err(PtrError::Value(e)),
                    Err(blob::Error::Padding) => {
                        todo!()
                    },
                }
            },
        }
        */ todo!()
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> PtrResult<Own<T::Owned, Self>, T, Self> {
        /*
        match Self::try_take_dirty(ptr) {
            Ok(this) => Ok(Own { this, zone: *self }),
            Err(fatptr) => {
                let valid_ptr = unsafe { ValidPtr::new_unchecked(fatptr) };
                let blob = self.get_blob(&valid_ptr).map_err(|e| PtrError::Ptr(e))?;

                match T::validate(blob.into_validator()) {
                    Ok(valid_blob) => unsafe {
                        let src: &ManuallyDrop<T> = &*(valid_blob.to_ref() as *const _ as *const _);

                        Ok(Own {
                            this: T::to_owned(src),
                            zone: *self,
                        })
                    },
                    Err(blob::Error::Value(e)) => Err(PtrError::Value(e)),
                    Err(blob::Error::Padding) => {
                        todo!()
                    },
                }
            },
        }*/ todo!()
    }
}

// Slow path, so put the actual implementation in its own function.
fn try_make_mut_impl<'a, 'p, 'v, T>(
    pile: &PileMut<'p,'v>,
    ptr: &'a mut ValidPtr<T, PileMut<'p,'v>>
) -> PtrResult<RefMut<'a, T, PileMut<'p,'v>>, T, PileMut<'p,'v>>
where T: ?Sized + Load<PileMut<'p,'v>>
{
    unsafe {
        let raw: *mut OffsetMut = ptr.raw_mut();

        let r = pile.try_get(ptr)?;
        let r: &ManuallyDrop<T> = &*(r.this as *const _ as *const _);
        let owned: T::Owned = T::to_owned(r);

        let new_ptr = pile.clone().alloc(owned).into_inner().into_inner();
        assert_eq!(new_ptr.metadata, ptr.metadata);

        raw.write(new_ptr.raw);

        match ptr.raw.kind() {
            offset::Kind::Ptr(nonnull) => {
                Ok(RefMut {
                    this: &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata),
                    zone: *pile,
                })
            },
            offset::Kind::Offset(_) => unreachable!("alloc should have returned a newly allocated ptr"),
        }
    }
}

#[cold]
fn try_make_mut<'a, 'p, 'v, T>(
    pile: &PileMut<'p,'v>,
    ptr: &'a mut ValidPtr<T, PileMut<'p,'v>>
) -> PtrResult<RefMut<'a, T, PileMut<'p,'v>>, T, PileMut<'p,'v>>
where T: ?Sized + Load<PileMut<'p,'v>>
{
    try_make_mut_impl(pile, ptr)
}

impl<'p, 'v> TryGetMut for PileMut<'p, 'v> {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>) -> PtrResult<RefMut<'a, T, Self>, T, Self> {
        match ptr.raw.kind() {
            offset::Kind::Ptr(nonnull) => {
                Ok(RefMut {
                    this: unsafe { &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata) },
                    zone: *self,
                })
            },
            offset::Kind::Offset(_) => try_make_mut(self, ptr),
        }
    }
}

fn handle_deref_error<'p,'v,E>(
    pile: Pile<'p,'v>,
    offset: Offset<'p,'v>,
    err: PtrError<E, OffsetError<'p,'v>>,
) -> !
where E: ValidationError
{
    let err = match err {
        PtrError::Ptr(err) => DerefError::Offset(err),
        PtrError::Value(err) => {
            let err: Box<dyn ValidationError> = Box::new(err);
            DerefError::Value { pile, offset, err }
        },
    };
    pile.mapping.handle_deref_error(err)
}

impl<'p, 'v> Get for Pile<'p, 'v> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self> {
        match self.try_get(ptr) {
            Ok(r) => r,
            Err(e) => handle_deref_error(*self, ptr.raw, e),
        }
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self> {
        let offset = ptr.raw;
        match self.try_take(ptr) {
            Ok(r) => r,
            Err(e) => handle_deref_error(*self, offset, e),
        }
    }
}

impl<'p, 'v> Get for PileMut<'p, 'v> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self> {
        match self.try_get(ptr) {
            Ok(r) => r,
            Err(e) => {
                let offset = ptr.raw.get_offset().expect("only offsets can fail");
                handle_deref_error(self.0, offset, e)
            },
        }
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self> {
        let raw = ptr.raw;
        match self.try_take(ptr) {
            Ok(r) => r,
            Err(e) => {
                let offset = raw.get_offset().expect("only offsets can fail");
                handle_deref_error(self.0, offset, e)
            },
        }
    }
}

// Slow path, so put the actual implementation in its own function.
#[inline(never)]
fn make_mut<'a, 'p, 'v, T>(
    pile: &PileMut<'p,'v>,
    ptr: &'a mut ValidPtr<T, PileMut<'p,'v>>
) -> RefMut<'a, T, PileMut<'p,'v>>
where T: ?Sized + Load<PileMut<'p,'v>>
{
    let offset = ptr.raw;
    match try_make_mut_impl(pile, ptr) {
        Ok(r) => r,
        Err(e) => {
            let offset = offset.get_offset().expect("only offsets can fail");
            handle_deref_error(pile.0, offset, e)
        },
    }
}

impl<'p, 'v> GetMut for PileMut<'p, 'v> {
    fn get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>) -> RefMut<'a, T, Self> {
        match ptr.raw.kind() {
            offset::Kind::Ptr(nonnull) => {
                RefMut {
                    this: unsafe { &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata) },
                    zone: *self,
                }
            },
            offset::Kind::Offset(_) => make_mut(self, ptr),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FullyValidateError<E> {
    /// The pile is smaller than the size of the tip.
    Undersized,

    /// Validation of the tip itself failed.
    Tip(blob::Error<E>),

    //Child(Error),
}


unsafe impl<'p,'v> PtrValidator<Self> for Pile<'p, 'v> {
    type Error = ValidatorError<'p, 'v>;

    fn validate_ptr<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a FatPtr<T, Pile<'static, 'static>>)
        -> Result<Option<T::ValidateChildren>, Self::Error>
    {
        /*
        let blob = self.get_blob(ptr)
                       .map_err(|err| ValidatorError::Offset { offset: err.offset })?;

        match T::validate(blob.into_validator()) {
            Ok(valid_blob) => {
                let value = unsafe { valid_blob.to_ref() };
                Ok(Some(value.validate_children()))
            },
            Err(blob::Error::Value(e)) => Err(
                ValidatorError::Value {
                    offset: ptr.raw,
                    err: Box::new(e),
                }
            ),
            Err(blob::Error::Padding) => Err(
                ValidatorError::Padding {
                    offset: ptr.raw,
                }
            ),
        }*/ todo!()
    }
}



unsafe impl<'p,'v> PtrValidator<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    type Error = ValidatePtrError;

    fn validate_ptr<'a, T>(&self, ptr: &'a FatPtr<T, Offset<'p,'v>>)
        -> Result<Option<T::ChildValidator>, Self::Error>
    where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        let blob = self.get_blob(ptr)
                       .map_err(ValidatePtrError::Offset)?;

        T::validate_blob(blob)
            .map(|blob_validator| Some(blob_validator.into_state()))
            .map_err(|e| ValidatePtrError::Value(Box::new(e)))
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
*/

impl<'p, 'v> PileMut<'p, 'v> {
    /*
    /// Creates a new version of an existing `PileMut`.
    ///
    /// Forwards to `Pile::update()`.
    pub fn update<'v2>(&self, new_slice: Unique<'v2, &'p &[u8]>) -> PileMut<'p, 'v2>
        where 'v: 'v2
    {
        self.0.update(new_slice).into()
    }

    /// Gets a blob from the pile.
    pub fn get_blob<'a, T>(&self, ptr: &'a FatPtr<T, Offset<'p, 'v>>) -> Result<Blob<'a, T>, offset::OffsetError>
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        self.get_slice(&ptr.raw, T::layout(ptr.metadata).size())
                  .map(|slice| Blob::new(slice, ptr.metadata).unwrap())
    }
    */

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
    pub fn save_to_vec<'a, T>(&self, value: &'a T) -> Vec<u8>
        where T: ?Sized + Save<'a, Self>
    {
        let mut state = value.save_children();

        let dumper = value.poll(&mut state, VecDumper::from(*self)).unwrap();
        let (dumper, _offset) = value.save_blob(&state, dumper).unwrap();

        dumper.into()
    }

    #[inline]
    fn min_align_layout(layout: Layout) -> Layout {
        unsafe {
            Layout::from_size_align_unchecked(
                layout.size(),
                cmp::min(layout.align(), 2),
            )
        }
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::PileMut;
    /// let pile = PileMut::default();
    /// ```
    #[inline]
    fn default() -> Self {
        PileMut::from(Pile::empty())
    }
}

impl<'p,'v> Alloc for PileMut<'p,'v> {
    type Zone = Self;

    fn alloc<T: ?Sized + Pointee>(&mut self, src: impl Take<T>) -> OwnedPtr<T, Self::Zone> {
        src.take_unsized(|src| unsafe {
            let layout = Self::min_align_layout(Layout::for_value(src));

            let ptr = if layout.size() > 0 {
                let dst = NonNull::new(std::alloc::alloc(layout))
                                  .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

                ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                    layout.size());

                dst.cast()
            } else {
                NonNull::new_unchecked(layout.align() as *mut u16)
            };

            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(
                    FatPtr {
                        raw: OffsetMut::from_ptr(ptr),
                        metadata: T::metadata(src),
                    }
            ))
        })
    }
}

/*
impl<'p,'v> GetMut<OffsetMut<'p,'v>> for PileMut<'p,'v> {
    #[inline]
    fn get_mut<'a, T>(&self, ptr: &'a mut ValidPtr<T, OffsetMut<'p,'v>>) -> RefMut<'a, T, OffsetMut<'p,'v>>
        where T: ?Sized + Load<OffsetMut<'p,'v>>
    {
        match ptr.raw.kind() {
            Kind::Ptr(nonnull) => {
                RefMut {
                    this: unsafe { &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata) },
                    zone: *self,
                }
            },
            Kind::Offset(_) => {
                // Slow path, so put the actual implementation in its own function.
                #[inline(never)]
                fn make_mut<'a, 'p, 'v, T>(
                    pile: &PileMut<'p,'v>,
                    ptr: &'a mut ValidPtr<T, OffsetMut<'p,'v>>
                ) -> &'a mut T
                where T: ?Sized + Load<OffsetMut<'p,'v>>
                {
                    // We create a copy of ptr instead of casting the reference because we need to
                    // modify it later, which the borrow checker wouldn't allow.
                    let offset: Offset = ptr.raw.try_cast().unwrap();
                    let ptr2 = FatPtr { raw: offset, metadata: ptr.metadata };
                    let ptr2 = unsafe { ValidPtr::new_unchecked(ptr2) };

                    // Get an owned copy of the value.
                    let blob = pile.load_blob(&ptr2);
                    let owned: T::Owned = T::decode_blob(blob, pile);

                    let new_ptr = pile.clone().alloc(owned).into_inner();

                    unsafe {
                        // Note how our ptr argument is a ValidPtr rather than OwnedPtr: if someone
                        // created a ValidPtr from scratch, the following would create a memory
                        // leak when the ValidPtr was dropped.
                        //
                        // TODO: maybe change the API of get_mut()?
                        *ptr.raw_mut() = new_ptr.raw;
                    }

                    match ptr.raw.kind() {
                        Kind::Ptr(nonnull) => {
                            unsafe {
                                &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata)
                            }
                        },
                        Kind::Offset(_) => unreachable!("alloc should have returned a newly allocated ptr"),
                    }
                }

                RefMut {
                    zone: *self,
                    this: make_mut(self, ptr),
                }
            },
        }
    }
}
*/

#[derive(Debug)]
struct VecDumper<'p, 'v> {
    pile: PileMut<'p, 'v>,
    buf: Vec<u8>,
}

impl<'p> Default for VecDumper<'p, 'static> {
    fn default() -> Self {
        Self {
            pile: PileMut::default(),
            buf: vec![],
        }
    }
}

impl<'p,'v> From<PileMut<'p,'v>> for VecDumper<'p,'v> {
    fn from(pile: PileMut<'p, 'v>) -> Self {
        Self { pile, buf: vec![] }
    }
}

impl<'p,'v> From<VecDumper<'p,'v>> for Vec<u8> {
    fn from(dumper: VecDumper<'p, 'v>) -> Self {
        dumper.buf
    }
}

impl<'p,'v> Dumper<PileMut<'p,'v>> for VecDumper<'p, 'v> {
    type Error = !;
    type PersistPtr = Offset<'static, 'static>;

    type WriteBlob = Vec<u8>;
    type WriteBlobOk = Vec<u8>;
    type WriteBlobError = !;

    fn try_save_ptr<'a, T: ?Sized + Pointee>(
        &self,
        ptr: &'a ValidPtr<T, PileMut<'p, 'v>>
    ) -> Result<Offset<'static, 'static>, &'a T>
    {
        match PileMut::try_get_dirty(ptr) {
            Ok(r) => Err(r),
            Err(ptr) => Ok(ptr.raw.cast()),
        }
    }

    fn save_blob(
        mut self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, Offset<'static, 'static>), !>
    {
        let offset = self.pile.slice().len() + self.buf.len();

        self.buf.reserve(size);
        self.buf = f(self.buf).unwrap();

        Ok((self, Offset::new(offset).unwrap()))
    }
}

/*
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
}
*/
