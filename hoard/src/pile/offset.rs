use core::any::Any;
use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::num::NonZeroU64;
use core::ptr::{self, NonNull};
use core::ops;

use std::alloc::Layout;

use leint::Le;

use super::*;

use crate::marshal::{
    Encode, Save, SavePtr,
    Persist,
    blob::*,
};

use crate::coerce::{TryCast, TryCastRef, TryCastMut};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'s, 'm> {
    marker: PhantomData<(&'s (), Snapshot<'m>)>,
    raw: Le<NonZeroU64>,
}

unsafe impl Persist for Offset<'_,'_> {}

impl fmt::Debug for Offset<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.raw.get().get() & 1 == 1);
        f.debug_tuple("Offset")
            .field(&self.get())
            .finish()
    }
}

impl From<Offset<'_, '_>> for usize {
    fn from(offset: Offset<'_,'_>) -> usize {
        offset.get()
    }
}

#[derive(Debug)]
pub struct OffsetError {
    offset: usize,
    size: usize,
}

impl<'s,'m> Offset<'s,'m> {
    pub const MAX: usize = (1 << 62) - 1;

    pub fn new(snap: &'s Snapshot<'m>, offset: usize, size: usize) -> Option<Self> {
        snap.get(offset .. offset + size)
            .map(|slice| unsafe { Self::new_unchecked(offset) })
    }

    pub unsafe fn new_unchecked(offset: usize) -> Self {
        let offset = offset as u64;
        Self {
            marker: PhantomData,
            raw: NonZeroU64::new((offset << 1) | 1).unwrap().into(),
        }
    }

    fn get_slice_from_pile<'a>(&'a self, size: usize, pile: &Pile<'s,'m>) -> Result<&'a [u8], OffsetError> {
        let snapshot: &Snapshot<'m> = unsafe { &*pile.snapshot.as_ptr() };

        let start = self.get();
        snapshot.get(start .. start + size).ok_or(OffsetError { offset: start, size })
                .map(|slice| {
                    // SAFETY: we can do this because we can only be created from a &'s [u8] slice.
                    let slice: &'s [u8] = unsafe { mem::transmute(slice) };
                    slice
                })
    }

    pub(super) fn get_blob_from_pile<'a, T>(ptr: &'a FatPtr<T, Self>, pile: &Pile<'s,'m>)
        -> Result<Blob<'a, T, Pile<'s,'m>>, OffsetError>
    where T: ?Sized + Load<Pile<'s,'m>>
    {
        let size = T::dyn_blob_layout(ptr.metadata).size();
        let slice = ptr.raw.get_slice_from_pile(size, pile).unwrap();

        Ok(Blob::new(slice, ptr.metadata).unwrap())
    }

    pub(super) fn load_valid_blob_from_pile<'a, T>(ptr: &'a ValidPtr<T, Self>, pile: &Pile<'s,'m>)
        -> Result<FullyValidBlob<'a, T, Pile<'s,'m>>, OffsetError>
    where T: ?Sized + Load<Pile<'s,'m>>
    {
        Self::get_blob_from_pile(ptr, pile)
            .map(|blob| unsafe { blob.assume_fully_valid() })
    }


    pub fn to_static(&self) -> Offset<'static, 'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    pub fn get(&self) -> usize {
        (self.raw.get().get() >> 1) as usize
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'s,'p>(Offset<'s,'p>);
unsafe impl Persist for OffsetMut<'_,'_> {}

unsafe impl<'s,'p> TryCastRef<OffsetMut<'s,'p>> for Offset<'s,'p> {
    type Error = !;

    #[inline(always)]
    fn try_cast_ref(&self) -> Result<&OffsetMut<'s,'p>, Self::Error> {
        Ok(unsafe { mem::transmute(self) })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TryCastOffsetMutError(());

unsafe impl<'s,'p> TryCastRef<Offset<'s,'p>> for OffsetMut<'s,'p> {
    type Error = TryCastOffsetMutError;

    #[inline]
    fn try_cast_ref(&self) -> Result<&Offset<'s,'p>, Self::Error> {
        match self.kind() {
            Kind::Offset(_) => Ok(&self.0),
            Kind::Ptr(_) => Err(TryCastOffsetMutError(())),
        }
    }
}

impl<'s, 'p> From<Offset<'s,'p>> for OffsetMut<'s,'p> {
    fn from(offset: Offset<'s,'p>) -> Self {
        Self(offset)
    }
}

impl Ptr for Offset<'_, '_> {
    fn dealloc_own<T: ?Sized + Pointee>(own: OwnedPtr<T, Self>) {
        let _ = own.into_inner();
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(_: OwnedPtr<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
    }
}

impl fmt::Pointer for Offset<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

unsafe impl<'s,'p> Encode<Pile<'s,'p>> for Offset<'s,'p> {
    fn blob_layout() -> BlobLayout {
        BlobLayout::new_nonzero(mem::size_of::<Self>())
    }

    type State = ();
    fn init_encode_state(&self) -> Self::State {}

    fn encode_poll<D: SavePtr<Pile<'s,'p>>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().get().to_le_bytes())?
           .finish()
    }

    fn encode_own<T: ?Sized + Save<Pile<'s,'p>>>(own: &OwnedPtr<T,Self>) -> Result<Self::State, <T as Save<Pile<'s,'p>>>::State> {
        Ok(())
    }
}

unsafe impl<'s,'p> Encode<PileMut<'s,'p>> for Offset<'s,'p> {
    fn blob_layout() -> BlobLayout {
        BlobLayout::new_nonzero(mem::size_of::<Self>())
    }

    type State = ();
    fn init_encode_state(&self) -> Self::State {}

    fn encode_poll<D: SavePtr<PileMut<'s,'p>>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().get().to_le_bytes())?
           .finish()
    }

    fn encode_own<T: ?Sized + Save<PileMut<'s,'p>>>(own: &OwnedPtr<T,Self>) -> Result<Self::State, <T as Save<PileMut<'s,'p>>>::State> {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeOffsetError {
    Ptr(u64),
    OutOfRange(u64),
}

impl<'s,'m> Decode<Pile<'s,'m>> for Offset<'s,'m> {
    type Error = DecodeOffsetError;

    type ValidateChildren = ();

    fn validate_blob<'a>(blob: Blob<'a, Self, Pile<'s,'m>>) -> Result<BlobValidator<'a, Self, Pile<'s,'m>>, Self::Error> {
        Self::ptr_validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Pile<'s,'m>>, _: &impl LoadPtr<Pile<'s,'m>>) -> Self {
        Self::ptr_decode_blob(blob)
    }

    /*
    fn deref_blob<'a>(blob: FullyValidBlob<'a, Self, Self>) -> &'a Self {
        unsafe { blob.assume_valid() }
    }

    fn ptr_validate_blob<'a>(blob: Blob<'a, Self, Self>) -> Result<FullyValidBlob<'a, Self, Self>, Self::Error> {
        let mut raw = [0;8];
        raw.copy_from_slice(&blob[..]);
        let raw = u64::from_le_bytes(raw);

        if raw & 1 != 1 {
            Err(DecodeOffsetError::Ptr(raw))
        } else {
            let offset = raw >> 1;
            Offset::new(offset).ok_or(DecodeOffsetError::OutOfRange(offset))?;

            unsafe { Ok(blob.assume_fully_valid()) }
        }
    }
    */

    fn ptr_decode_blob<'a>(blob: FullyValidBlob<'a, Self, Pile<'s,'m>>) -> Self {
        *<Self as Decode<Pile<'s,'m>>>::deref_blob(blob)
    }

    /*
    fn ptr_validate_children<T, V>(ptr: &FatPtr<T,Self>, validator: &mut V) -> Result<Option<T::ValidateChildren>, V::Error>
        where T: ?Sized + Load<Self>,
              V: ValidatePtr<Self>,
    {
        validator.validate_ptr(ptr)
    }
    */
}

impl<'s,'m> Decode<PileMut<'s,'m>> for Offset<'s,'m> {
    type Error = DecodeOffsetError;

    type ValidateChildren = ();

    fn validate_blob<'a>(blob: Blob<'a, Self, PileMut<'s,'m>>) -> Result<BlobValidator<'a, Self, PileMut<'s,'m>>, Self::Error> {
        Self::ptr_validate_blob(blob)?;
        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, PileMut<'s,'m>>, _: &impl LoadPtr<PileMut<'s,'m>>) -> Self {
        Self::ptr_decode_blob(blob)
    }
}

impl Ptr for OffsetMut<'_, '_> {
    fn dealloc_own<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            unsafe {
                core::ptr::drop_in_place(value)
            }
        )
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: OwnedPtr<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        let FatPtr { raw, metadata } = owned.into_inner().into_inner();

        match raw.kind() {
            Kind::Offset(_) => {},
            Kind::Ptr(ptr) => {
                unsafe {
                    let r: &mut T = &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                    let r: &mut ManuallyDrop<T> = &mut *(r as *mut _ as *mut _);

                    f(r);

                    let layout = fix_layout(Layout::for_value(r));
                    if layout.size() > 0 {
                        std::alloc::dealloc(r as *mut _ as *mut u8, layout);
                    }
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Kind<'s,'p> {
    Offset(Offset<'s,'p>),
    Ptr(NonNull<u16>),
}

fn fix_layout(layout: Layout) -> Layout {
    unsafe {
        Layout::from_size_align_unchecked(
            layout.size(),
            cmp::min(layout.align(), 2),
        )
    }
}

impl<'s,'m> OffsetMut<'s,'m> {
    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

    pub fn kind(&self) -> Kind<'s,'m> {
        if self.0.raw.get().get() & 1 == 1 {
            Kind::Offset(self.0)
        } else {
            Kind::Ptr(unsafe {
                let raw = self.0.raw.get().get();
                NonNull::new_unchecked(raw as usize as *mut u16)
            })
        }
    }

    pub(super) unsafe fn alloc<T: ?Sized + Pointee>(src: &ManuallyDrop<T>) -> Self {
        let layout = fix_layout(Layout::for_value(src));

        let ptr = if layout.size() > 0 {
            let dst = NonNull::new(std::alloc::alloc(layout))
                              .unwrap_or_else(|| std::alloc::handle_alloc_error(layout));

            ptr::copy_nonoverlapping(src as *const _ as *const u8, dst.as_ptr(),
                                layout.size());

            dst.cast()
        } else {
            NonNull::new_unchecked(layout.align() as *mut u16)
        };

        Self::from_ptr(ptr)
    }


    pub(super) unsafe fn try_take<T: ?Sized + Pointee + Owned>(self, metadata: T::Metadata) -> Result<T::Owned, Offset<'s,'m>> {
        let this = ManuallyDrop::new(self);

        match this.kind() {
            Kind::Offset(offset) => Err(offset),
            Kind::Ptr(ptr) => {
                let ptr: *mut T = T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                let r = &mut *(ptr as *mut ManuallyDrop<T>);
                let layout = fix_layout(Layout::for_value(r));

                let owned = T::to_owned(r);

                if layout.size() > 0 {
                    std::alloc::dealloc(r as *mut _ as *mut u8, layout);
                }

                Ok(owned)
            }
        }
    }

    pub(super) fn get_blob_from_pile<'a, T>(ptr: &'a FatPtr<T, Offset<'s,'m>>, pile: &PileMut<'s,'m>)
        -> Result<Blob<'a, T, PileMut<'s,'m>>, OffsetError>
    where T: ?Sized + Load<PileMut<'s,'m>>
    {
        let size = T::dyn_blob_layout(ptr.metadata).size();
        let slice = ptr.raw.get_slice_from_pile(size, pile).unwrap();

        Ok(Blob::new(slice, ptr.metadata).unwrap())
    }

    pub(super) fn load_valid_blob_from_pile<'a, T>(ptr: &'a ValidPtr<T, Offset<'s,'m>>, pile: &PileMut<'s,'m>)
        -> Result<FullyValidBlob<'a, T, PileMut<'s,'m>>, OffsetError>
    where T: ?Sized + Load<PileMut<'s,'m>>
    {
        Self::get_blob_from_pile(ptr, pile)
            .map(|blob| unsafe { blob.assume_fully_valid() })
    }
}

impl fmt::Debug for OffsetMut<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.kind(), f)
    }
}

impl fmt::Pointer for OffsetMut<'_,'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
