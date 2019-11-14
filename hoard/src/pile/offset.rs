use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::num::NonZeroU64;
use core::ptr::{self, NonNull};

use std::alloc::Layout;

use super::*;

use crate::marshal::{
    Encode, EncodePoll,
    Decode, ValidateChildren,
    Save, Load, Loader,
    Persist,
    blob::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,

    // FIXME: needs to be Le<NonZeroU64>
    raw: NonZeroU64,
}

unsafe impl Persist for Offset<'_> {}

impl<'p> Offset<'p> {
    pub const MAX: u64 = (1 << 62) - 1;

    pub fn new(offset: u64) -> Option<Self> {
        if offset <= Self::MAX {
            Some(Self {
                marker: PhantomData,
                raw: NonZeroU64::new((offset << 1) | 1).unwrap(),
            })
        } else {
            None
        }
    }

    pub fn persist(self) -> Offset<'static> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }

    pub fn get(self) -> u64 {
        self.raw.get() >> 1
    }

    pub unsafe fn coerce<'q>(self) -> Offset<'q> {
        Offset {
            marker: PhantomData,
            raw: self.raw,
        }
    }
}

impl Ptr for Offset<'_> {
    fn dealloc_own<T: ?Sized + Pointee>(own: Own<T, Self>) {
        let (_, _) = own.into_raw_parts();
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(_: Own<T, Self>, _: impl FnOnce(&mut ManuallyDrop<T>)) {
    }
}

impl fmt::Pointer for Offset<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

impl Encode<Self> for Offset<'_> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type EncodePoll = Self;
    fn encode_poll(self) -> Self { self }

    fn encode_own<T: ?Sized>(own: Own<T,Self>) -> Result<Self::EncodePoll, <T as Save<Self>>::SavePoll>
        where T: Save<Self>
    {
        let (this, _) = own.into_raw_parts();
        Ok(this)
    }
}

impl EncodePoll<Self> for Offset<'_> {
    const TARGET_BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type Target = Self;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().to_le_bytes())?
           .finish()
    }
}

#[derive(Debug)]
pub struct OffsetError(());

impl TryFrom<u64> for Offset<'static> {
    type Error = OffsetError;

    fn try_from(raw: u64) -> Result<Self, Self::Error> {
        Offset::new(raw).ok_or(OffsetError(()))
    }
}

impl Decode<Self> for Offset<'_> {
    type Error = OffsetError;

    type ValidateChildren = ();

    fn validate_blob<'q>(blob: Blob<'q, Self, Self>) -> Result<BlobValidator<'q, Self, Self>, Self::Error> {
        let mut raw = [0;8];
        raw.copy_from_slice(&blob[..]);
        let raw = u64::from_le_bytes(raw);

        if raw & 1 != 1 {
            Err(OffsetError(()))
        } else {
            Offset::try_from(raw >> 1)?;

            //Ok(blob.assume_valid(()))
            todo!()
        }
    }

    fn decode_blob<'q>(blob: FullyValidBlob<'q, Self, Self>, _: &impl Loader<Self>) -> Self {
        *<Self as Decode<Self>>::deref_blob(blob)
    }

    fn deref_blob<'q>(blob: FullyValidBlob<'q, Self, Self>) -> &'q Self {
        unsafe { blob.assume_valid() }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p>(Offset<'p>);

unsafe impl Persist for OffsetMut<'_> {}

impl Ptr for OffsetMut<'_> {
    fn dealloc_own<T: ?Sized + Pointee>(owned: Own<T, Self>) {
        Self::drop_take_unsized(owned, |value|
            unsafe {
                core::ptr::drop_in_place(value)
            }
        )
    }

    fn drop_take_unsized<T: ?Sized + Pointee>(owned: Own<T, Self>, f: impl FnOnce(&mut ManuallyDrop<T>)) {
        let (this, metadata) = owned.into_raw_parts();
        let this = ManuallyDrop::new(this);

        match this.kind() {
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
pub enum Kind<'p> {
    Offset(Offset<'p>),
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

impl<'p> OffsetMut<'p> {
    pub fn from_offset(offset: Offset<'p>) -> Self {
        Self(offset)
    }

    pub unsafe fn from_ptr(ptr: NonNull<u16>) -> Self {
        let raw = ptr.as_ptr() as usize as u64;

        assert_eq!(raw & 1, 0,
                   "{:p} unaligned", ptr);

        mem::transmute(ptr.as_ptr() as usize as u64)
    }

    pub fn kind(&self) -> Kind<'p> {
        match self.0.raw.get() & 1 {
            1 => Kind::Offset(self.0),
            0 => Kind::Ptr(unsafe {
                let raw = self.0.raw.get();
                NonNull::new_unchecked(raw as usize as *mut u16)
            }),
            _ => unreachable!(),
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


    pub(super) unsafe fn try_take<T: ?Sized + Pointee + Owned>(self, metadata: T::Metadata) -> Result<T::Owned, Offset<'p>> {
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
}

impl fmt::Debug for OffsetMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.kind(), f)
    }
}

impl fmt::Pointer for OffsetMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind() {
            Kind::Offset(offset) => write!(f, "Offset({:p})", offset),
            Kind::Ptr(ptr) => write!(f, "Ptr({:p})", ptr),
        }
    }
}

impl Encode<Self> for OffsetMut<'_> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type EncodePoll = Self;
    fn encode_poll(self) -> Self { self }

    fn encode_own<T: ?Sized>(own: Own<T,Self>) -> Result<Self::EncodePoll, <T as Save<Self>>::SavePoll>
        where T: Save<Self>
    {
        todo!()
    }
}

impl EncodePoll<Self> for OffsetMut<'_> {
    const TARGET_BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type Target = Self;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }
}

impl Decode<Self> for OffsetMut<'_> {
    type Error = OffsetError;

    type ValidateChildren = ();

    fn validate_blob<'q>(blob: Blob<'q, Self, Self>) -> Result<BlobValidator<'q, Self, Self>, Self::Error> {
        todo!()
    }

    fn decode_blob<'q>(blob: FullyValidBlob<'q, Self, Self>, _: &impl Loader<Self>) -> Self {
        let inner = <Self as Decode<Self>>::deref_blob(blob).0;
        // TODO: add assertion
        Self(inner)
    }

    fn deref_blob<'q>(blob: FullyValidBlob<'q, Self, Self>) -> &'q Self {
        unsafe { blob.assume_valid() }
    }
}
