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

use std::any::type_name;
use std::cmp;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops;
use std::ptr::{self, NonNull};

use std::alloc::Layout;

use owned::{Take, IntoOwned};
use singlelife::Unique;

use crate::coerce::{Coerce, TryCoerce};
use crate::pointee::Pointee;
use crate::zone::{*, refs::*};
use crate::marshal::decode::*;
use crate::marshal::load::*;
use crate::marshal::blob::{self, Blob, ValidBlob, Validate};
use crate::marshal::PtrValidator;

pub mod offset;
use self::offset::Offset;

pub mod offsetmut;
use self::offsetmut::OffsetMut;

pub mod error;
use self::error::*;

pub mod mapping;
use self::mapping::Mapping;

/// Fallible, unverified, `Pile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TryPile<'pile, 'version> {
    marker: PhantomData<
        fn(&Offset<'pile, 'version>) -> &'pile [u8]
    >,
    mapping: &'pile dyn Mapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pile<'pile, 'version>(TryPile<'pile, 'version>);

/// Mutable, unverified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TryPileMut<'p, 'v>(TryPile<'p, 'v>);

/// Mutable, unverified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileMut<'p, 'v>(TryPileMut<'p, 'v>);

impl<'p, 'v> From<TryPile<'p, 'v>> for Pile<'p,'v> {
    fn from(trypile: TryPile<'p,'v>) -> Self {
        Self(trypile)
    }
}

impl<'p, 'v> From<Pile<'p, 'v>> for TryPile<'p,'v> {
    fn from(pile: Pile<'p,'v>) -> Self {
        pile.0
    }
}


impl<'p,'v> ops::Deref for Pile<'p,'v> {
    type Target = TryPile<'p,'v>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'p> From<Unique<'p, &&[u8]>> for TryPile<'p, 'p> {
    #[inline]
    fn from(slice: Unique<'p, &&[u8]>) -> Self {
        Self {
            marker: PhantomData,
            mapping: &*Unique::into_inner(slice),
        }
    }
}

pub trait PileZone<'p, 'v>
: Zone<Error = Error<'p,'v>,
       PersistPtr = Offset<'static, 'static>>
{
    fn get_try_pile(&self) -> TryPile<'p, 'v>;

    fn mapping(&self) -> &'p dyn Mapping;

    fn slice(&self) -> &'p &'p [u8] {
        unsafe {
            &*(self.mapping() as *const dyn Mapping as *const &'p [u8])
        }
    }
}

pub trait PileZoneMut<'p, 'v> : PileZone<'p, 'v> + Zone<Ptr = OffsetMut<'p,'v>>
{
}

impl<'p, 'v> PileZone<'p, 'v> for TryPile<'p, 'v> {
    fn get_try_pile(&self) -> TryPile<'p, 'v> {
        *self
    }

    fn mapping(&self) -> &'p dyn Mapping {
        self.mapping
    }
}

impl<'p, 'v> PileZone<'p, 'v> for TryPileMut<'p, 'v> {
    fn get_try_pile(&self) -> TryPile<'p, 'v> {
        self.0
    }

    fn mapping(&self) -> &'p dyn Mapping {
        self.0.mapping
    }
}

impl<'p, 'v> PileZoneMut<'p, 'v> for TryPileMut<'p, 'v> {}

impl<'p, 'v1, 'v2, T> AsRef<FatPtr<T, TryPile<'p, 'v2>>> for FatPtr<T, TryPile<'p, 'v1>>
where T: ?Sized + Pointee,
{
    fn as_ref(&self) -> &FatPtr<T, TryPile<'p, 'v2>> {
        unsafe { mem::transmute(self) }
    }
}

impl TryPile<'_, '_> {
    /// Creates a new `TryPile` from a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// # use leint::Le;
    /// TryPile::new([0x12, 0x34, 0x56, 0x78], |pile| {
    ///     let tip = pile.try_get_tip::<Le<u32>>().unwrap();
    ///     assert_eq!(**tip, 0x78563412);
    /// })
    /// ```
    #[inline]
    pub fn new<R>(slice: impl AsRef<[u8]>, f: impl FnOnce(TryPile) -> R) -> R {
        let slice = slice.as_ref();
        Unique::new(&slice, |slice| {
            f(TryPile::from(slice))
        })
    }
}

impl TryPile<'_, 'static> {
    /// Creates an empty `TryPile`.
    ///
    /// Note how the `'version` parameter is `'static': the earliest possible version of a pile is
    /// to have nothing in it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// let empty = TryPile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert!(empty.try_get_tip::<u8>().is_err());
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.try_get_tip::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        static EMPTY_SLICE: &[u8] = &[];

        TryPile {
            marker: PhantomData,
            mapping: &EMPTY_SLICE,
        }
    }
}

impl<'p,'v> TryPile<'p, 'v> {
    pub fn new_valid_ptr<T: ?Sized + Pointee>(offset: usize, metadata: T::Metadata) -> ValidPtr<T, Self> {
        let raw = Offset::new(offset).unwrap();

        // Safe because our pointers have no special meaning.
        unsafe { ValidPtr::new_unchecked(FatPtr { raw, metadata }) }
    }

    /// Tries to get the tip of a `TryPile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::TryPile;
    /// # use leint::Le;
    /// TryPile::new(&[42], |pile| {
    ///     let tip = pile.try_get_tip::<u8>().unwrap();
    ///     assert_eq!(**tip, 42u8);
    ///
    ///     // Fails, because the slice is too small to be a valid Le<u32>
    ///     assert!(pile.try_get_tip::<Le<u32>>().is_err());
    ///
    ///     // Fails, because the slice isn't a valid bool
    ///     assert!(pile.try_get_tip::<bool>().is_err());
    /// })
    /// ```
    pub fn try_get_tip<T: Decode<Self>>(&self) -> Result<Ref<'p, T, Self>, Error<'p,'v>> {
        // By using saturating_sub we don't have to handle the too-large case ourselves.
        let offset = self.slice().len().saturating_sub(mem::size_of::<T>());

        let ptr = FatPtr::<T,_> {
            raw: Offset::new(offset.into()).unwrap(),
            metadata: ()
        };
        let r = try_get_impl(self, &ptr)?;
        Ok(Ref {
            this: unsafe { T::assume_valid_ref(r) },
            zone: *self,
        })
    }
}

/// ```
/// dbg!(true);
/// ```
pub fn get_blob_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &FatPtr<T, Z::Persist>,
) -> Result<Blob<'a, T::Persist>, Error<'p,'v>>
where T: ?Sized + PersistPointee,
      Z: PileZone<'p,'v>,
{
    let layout = T::try_layout(ptr.metadata)
                   .map_err(|e| Error::new(zone, ptr, ErrorKind::Metadata(e.into())))?;

    // It's impossible for this to overflow as the maximum offset is just a quarter of
    // usize::MAX
    let start = ptr.raw.get();
    let end = start + layout.size();
    match zone.slice().get(start .. end) {
        None => Err(Error::new(zone, ptr, ErrorKind::Offset)),
        Some(slice) => {
            let ptr = T::Persist::make_fat_ptr(slice.as_ptr() as *const (), ptr.metadata);

            unsafe {
                assert_eq!(layout.align(), 1,
                           "PersistPointee can't be implemented for aligned type {}", type_name::<T>());
                Ok(Blob::from_ptr(ptr))
            }
        },
    }
}

fn try_get_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &FatPtr<T, Z::Persist>,
)
-> Result<&'a T::Persist, Error<'p, 'v>>
where T: ?Sized + PersistPointee,
      Z: PileZone<'p,'v>,
{
    let blob = get_blob_impl(zone, ptr)?;

    let cursor = blob.into_cursor_ignore_padding();
    match T::Persist::validate(cursor) {
        Ok(valid_blob) => Ok(valid_blob.to_ref()),
        Err(blob::Error::Error(err)) => Err(Error::new(zone, ptr, ErrorKind::Value(err.into()))),
        Err(blob::Error::Padding(never)) => match never {},
    }
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
        TryPile::new(slice, |try_pile| {
            f(Pile::from(try_pile))
        })
    }
}

impl Pile<'_, 'static> {
    /// Creates an empty `Pile`.
    ///
    /// Note how the `'version` parameter is `'static': the earliest possible version of a pile is
    /// to have nothing in it.
    ///
    /// # Examples
    ///
    /// ```todo
    /// # use hoard::pile::{Pile, TipError};
    /// let empty = Pile::empty();
    ///
    /// // Attempting to load anything from an empty pile fails, as there's nothing there...
    /// assert_eq!(empty.fully_validate_tip::<u8>().unwrap_err(),
    ///            TipError::Undersized);
    ///
    /// // ...with the exception of zero-sized types!
    /// empty.fully_validate_tip::<()>().unwrap();
    /// ```
    #[inline]
    pub fn empty() -> Self {
        TryPile::empty().into()
    }
}

/*
impl<'p, 'v> Pile<'p, 'v> {
    /// Loads the tip of the `Pile`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use hoard::pile::Pile;
    /// Pile::new([12], |pile| {
    ///     let tip = pile.fully_validate_tip::<u8>().unwrap();
    ///     assert_eq!(**tip, 12);
    /// })
    /// ```
    pub fn fully_validate_tip<T: Decode<Self>>(&self)
        -> Result<Ref<'p, T, Self>, TipError<T::Error>>
    {
        let metadata = T::make_sized_metadata();
        let size = T::blob_size(metadata);
        let offset = self.slice().len()
                         .saturating_sub(size);

        let blob = self.get_blob_slice::<T>(Offset::new(offset).unwrap(), metadata)
                       .map(|slice| Blob::new(slice, metadata).unwrap())
                       .ok_or(TipError::Undersized)?;

        let r = T::validate_blob(blob.into_validator())
                    .map_err(|e| TipError::Tip(e))?
                    .to_ref();

        let mut state = T::validate_children(r);

        match T::poll(r, &mut state, &mut self.duplicate()) {
            Ok(this) => Ok(Ref { this, zone: *self }),
            Err(err) => {
                todo!()
            }
        }
    }

    /*
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
    */

}
*/

/// Validates piles fully.
#[derive(Debug)]
pub struct FullValidator<'p,'v, Z> {
    marker: PhantomData<TryPile<'p,'v>>,
    pile: Z,
}

impl<'p, 'v, Z> PtrValidator<Z> for FullValidator<'p, 'v, Z>
where Z: PileZone<'p, 'v>
{
    type Error = Error<'p,'v>;

    fn validate_ptr<'a, T: ?Sized>(
        &self,
        ptr: &'a FatPtr<T::Persist, Z::Persist>
    ) -> Result<Option<&'a T::Persist>, Self::Error>
        where T: ValidatePointeeChildren<'a, Z>
    {
        //let blob = get_blob_impl(&self.pile, ptr)?;

        /*
        match T::validate_blob(blob.into_validator()) {
            Ok(valid_blob) => Ok(Some(valid_blob.to_ref())),
            Err(e) => Err(PtrValidatorError::with_error(ptr, e)),
        }
        */ todo!()
    }
}

impl<'p,'v> Zone for TryPile<'p,'v> {
    type Ptr = Offset<'p,'v>;
    type Persist = TryPile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = Error<'p,'v>;

    #[inline(always)]
    fn duplicate(&self) -> Self {
        *self
    }

    fn clone_ptr<T>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        unsafe { OwnedPtr::new_unchecked(ValidPtr::new_unchecked(**ptr)) }
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        Err(FatPtr {
            raw: ptr.raw.cast(),
            metadata: ptr.metadata,
        })
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let fat = owned.into_inner().into_inner();
        f(Err(FatPtr {
            raw: fat.raw.cast(),
            metadata: fat.metadata,
        }))
    }
}

impl<'p, 'v> TryGet for TryPile<'p, 'v> {
    fn try_get<'a, T>(&self, ptr: &'a ValidPtr<T, Self>) -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        let ptr: FatPtr<T, Self> = **ptr;
        let ptr: FatPtr<T, TryPile<'static, 'static>> = ptr.coerce();
        let r_persist = try_get_impl(self, &ptr)?;
        Ok(Ref {
            this: unsafe { T::assume_valid_ref(r_persist) },
            zone: *self,
        })
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        let ptr: FatPtr<T, Self> = **ptr;
        let ptr: FatPtr<T, TryPile<'static, 'static>> = ptr.coerce();
        let r_persist = try_get_impl(self, &ptr)?;

        Ok(Own {
            this: unsafe { T::assume_valid(r_persist) },
            zone: *self,
        })
    }
}

pub fn test_trypile<'a,'p,'v>(pile: &TryPile<'p,'v>,
    ptr1: &[ValidPtr<[bool;2], TryPile<'p, 'v>>; 100],
    ptr2: &[ValidPtr<u8, TryPile<'p, 'v>>; 100],
) -> Result<usize, Error<'p,'v>>
{
    let mut sum = 0;

    for (ptr1, ptr2) in ptr1.iter().zip(ptr2.iter()) {
        let [a,b] = **pile.try_get(ptr1)?;
        if a != b {
            let n = pile.try_get(ptr2)?;
            sum += **n as usize;
        }
    }
    Ok(sum)
}

/*
impl<'p,'v> Zone for Pile<'p,'v> {
    type Ptr = Offset<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;
    type Allocator = NeverAllocator<Self>;

    type Error = !;

    #[inline(always)]
    fn duplicate(&self) -> Self {
        *self
    }

    fn clone_ptr<T>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        unsafe {
            OwnedPtr::new_unchecked(ValidPtr::new_unchecked(**ptr))
        }
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        Err(FatPtr {
            raw: ptr.raw.cast(),
            metadata: ptr.metadata,
        })
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let fat = owned.into_inner().into_inner();
        f(Err(FatPtr {
            raw: fat.raw.cast(),
            metadata: fat.metadata,
        }))
    }
}
impl<'p, 'v> Get for Pile<'p, 'v> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self> {
        let ptr = unsafe { &*(ptr as *const _ as *const ValidPtr<T, TryPile<'p, 'v>>) };
        match self.try_get_impl(ptr) {
            Ok(r) => {
                Ref {
                    this: r.this,
                    zone: *self,
                }
            },
            Err(e) => todo!("{:?}", e), //handle_deref_error(*self, ptr.raw, e),
        }
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self> {
        let r = self.get(&ptr);

        unsafe {
            let r: &ManuallyDrop<T> = &*(r.this as *const _ as *const _);
            Own {
                this: T::into_owned(r),
                zone: *self,
            }
        }
    }
}
*/


impl<'p> Default for TryPileMut<'p, 'static> {
    fn default() -> Self {
        TryPileMut(TryPile::empty())
    }
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

impl<'p,'v> Zone for TryPileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;
    type Persist = TryPile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;

    type Error = Error<'p,'v>;

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }

    #[inline(always)]
    fn duplicate(&self) -> Self {
        Self(self.0)
    }

    fn clone_ptr<T>(ptr: &ValidPtr<T, Self>) -> OwnedPtr<T, Self> {
        todo!()
    }

    fn try_get_dirty<T: ?Sized + Pointee>(ptr: &ValidPtr<T, Self>) -> Result<&T, FatPtr<T, Self::Persist>> {
        match ptr.raw.kind() {
            offsetmut::Kind::Ptr(nonnull) => unsafe {
                Ok(&*T::make_fat_ptr(nonnull.cast().as_ptr(), ptr.metadata))
            },
            offsetmut::Kind::Offset(raw) => {
                let raw = raw.cast();
                Err(FatPtr { raw, metadata: ptr.metadata })
            },
        }
    }

    fn try_take_dirty_unsized<T: ?Sized + Pointee, R>(
        owned: OwnedPtr<T, Self>,
        f: impl FnOnce(Result<&mut ManuallyDrop<T>, FatPtr<T, Self::Persist>>) -> R,
    ) -> R
    {
        let metadata = owned.metadata;
        OffsetMut::try_take_dirty_unsized(owned, |r|
            f(match r {
                Ok(t_ref) => Ok(t_ref),
                Err(offset) => Err(FatPtr { raw: offset.cast(), metadata }),
            })
        )
    }
}

impl<'p,'v> Alloc for TryPileMut<'p,'v> {
    fn alloc<T: ?Sized + Pointee>(&self, src: impl Take<T>) -> OwnedPtr<T, Self> {
        OffsetMut::alloc(src)
    }
}

impl<'p, 'v> TryGet for TryPileMut<'p, 'v> {
    fn try_get<'a, T>(&self, ptr: &'a ValidPtr<T, Self>) -> Result<Ref<'a, T, Self>, Self::Error>
        where T: ?Sized + PersistPointee
    {
        let fatptr: FatPtr<T, Self> = **ptr;

        match TryCoerce::<FatPtr<T, TryPile>>::try_coerce(**ptr) {
            Ok(ptr) => {
                let r_persist = try_get_impl(self, &ptr)?;
                Ok(Ref {
                    this: unsafe { T::assume_valid_ref(r_persist) },
                    zone: *self,
                })
            },
            Err(err) => unsafe {
                let r: &T = &*T::make_fat_ptr(err.ptr.cast().as_ptr(), ptr.metadata);
                Ok(Ref {
                    this: r,
                    zone: self.duplicate()
                })
            },
        }
    }

    fn try_take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>)
        -> Result<Own<T::Owned, Self>, Self::Error>
    {
        let metadata: T::Metadata = ptr.metadata;
        OffsetMut::try_take_dirty_unsized(ptr, |result| {
            match result {
                Ok(dirty) => {
                    Ok(Own {
                        this: unsafe { T::into_owned_unchecked(dirty) },
                        zone: *self,
                    })
                },
                Err(offset) => {
                    let ptr = FatPtr::<T, TryPile> {
                        raw: offset.coerce(),
                        metadata,
                    };
                    let r_persist = try_get_impl(self, &ptr)?;
                    Ok(Own {
                        this: unsafe { T::assume_valid(r_persist) },
                        zone: *self,
                    })
                },
            }
        })
    }
}

fn try_get_mut_impl<'a, 'p: 'a, 'v, T, Z>(
    zone: &Z,
    ptr: &'a mut ValidPtr<T,Z>,
)
-> Result<RefMut<'a, T, Z>, Error<'p, 'v>>
where T: ?Sized + PersistPointee,
      Z: PileZoneMut<'p,'v> + Alloc,
{
    match TryCoerce::<FatPtr<T, Z::Persist>>::try_coerce(**ptr) {
        Err(err) => {
            // Pointer is dirty, so we can just dereference it.
            let r: &mut T = unsafe { &mut *T::make_fat_ptr_mut(err.ptr.cast().as_ptr(), ptr.metadata) };
            Ok(RefMut {
                this: r,
                zone: zone.duplicate()
            })
        },
        Ok(persist_ptr) => {
            let r = try_get_impl(zone, &persist_ptr)?;

            // FIXME: double conversion, eg [u8] -> Vec[u8] -> [u8]
            //
            // Maybe assume_valid_alloc()?
            let owned = unsafe { T::assume_valid(r) };

            let new_ptr: FatPtr<T, Z> = zone.alloc(owned).into_inner().into_inner();

            // SAFETY: ValidPtr::raw_mut() requires us to maintain the validity of the pointer.
            // new_ptr is freshly allocated, so it should be valid as long as the metadata is
            // unchanged.
            assert_eq!(ptr.metadata, new_ptr.metadata);
            let old_raw = unsafe { mem::replace(ptr.raw_mut(), new_ptr.raw) };

            // Make sure we're not leaking memory.
            assert!(old_raw.get_offset().is_some());

            if let Some(nonnull) = ptr.raw.get_ptr() {
                Ok(RefMut {
                    // SAFETY: We do in fact have mutable access here.
                    this: unsafe { &mut *T::make_fat_ptr_mut(nonnull.cast().as_ptr(), ptr.metadata) },
                    zone: zone.duplicate(),
                })
            } else {
                unreachable!("alloc should have returned a newly allocated ptr")
            }
        },
    }
}

impl<'p, 'v> TryGetMut for TryPileMut<'p, 'v> {
    fn try_get_mut<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a mut ValidPtr<T, Self>)
        -> Result<RefMut<'a, T, Self>, Self::Error>
    {
        try_get_mut_impl(self, ptr)
    }
}

/*
impl<'p,'v> Zone for PileMut<'p,'v> {
    type Ptr = OffsetMut<'p,'v>;
    type Persist = Pile<'static, 'static>;
    type PersistPtr = Offset<'static, 'static>;
    type Allocator = Self;

    type Error = !;

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
                            this: T::into_owned(src),
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
        let owned: T::Owned = T::into_owned(r);

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
*/

/*
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
*/

/*
impl<'p, 'v> Get for PileMut<'p, 'v> {
    fn get<'a, T: ?Sized + Load<Self>>(&self, ptr: &'a ValidPtr<T, Self>) -> Ref<'a, T, Self> {
        /*
        match self.try_get(ptr) {
            Ok(r) => r,
            Err(e) => {
                let offset = ptr.raw.get_offset().expect("only offsets can fail");
                handle_deref_error(self.0, offset, e)
            },
        }
        */ todo!()
    }

    fn take<T: ?Sized + Load<Self>>(&self, ptr: OwnedPtr<T, Self>) -> Own<T::Owned, Self> {
        /*
        let raw = ptr.raw;
        match self.try_take(ptr) {
            Ok(r) => r,
            Err(e) => {
                let offset = raw.get_offset().expect("only offsets can fail");
                handle_deref_error(self.0, offset, e)
            },
        }
        */ todo!()
    }
}
*/

/*
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
*/

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn trypile_alloc() {
        let pile = TryPileMut::default();

        let x = pile.alloc(42u8);

        //pile.try_get(&x);
    }
}
