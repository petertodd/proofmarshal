use core::any::Any;
use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::num::NonZeroU64;
use core::ptr::{self, NonNull};

use std::alloc::Layout;

use leint::Le;

use super::*;

use crate::marshal::{
    Encode, Save, Dumper,
    Persist,
    blob::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    raw: Le<NonZeroU64>,
}

unsafe impl Persist for Offset<'_> {}

impl<'p> Offset<'p> {
    pub const MAX: u64 = (1 << 62) - 1;

    pub fn new(offset: u64) -> Option<Self> {
        if offset <= Self::MAX {
            Some(Self {
                marker: PhantomData,
                raw: NonZeroU64::new((offset << 1) | 1).unwrap().into(),
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
        self.raw.get().get() >> 1
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
        let _ = own.into_inner();
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

    type State = ();
    fn init_encode_state(&self) -> Self::State {}

    fn encode_poll<D: Dumper<Self>>(&self, _: &mut (), dumper: D) -> Result<D, D::Pending> {
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, _: &(), dst: W) -> Result<W::Ok, W::Error> {
        dst.write_bytes(&self.raw.get().get().to_le_bytes())?
           .finish()
    }

    fn encode_own<T: ?Sized + Save<Self>>(own: &Own<T,Self>) -> Result<Self::State, <T as Save<Self>>::State> {
        Ok(())
    }

    fn encode_own_ptr<W: WriteBlob>(&self, _: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        self.encode_blob(&(), dst)
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

            Ok(blob.assume_valid(()))
        }
    }

    fn decode_blob<'q>(blob: FullyValidBlob<'q, Self, Self>, _: &impl LoadPtr<Self>) -> Self {
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
        let FatPtr { raw, metadata } = owned.into_inner();

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
        match self.0.raw.get().get() & 1 {
            1 => Kind::Offset(self.0),
            0 => Kind::Ptr(unsafe {
                let raw = self.0.raw.get().get();
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

impl<'p> Encode<Self> for OffsetMut<'p> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

    type State = Offset<'static>;
    fn init_encode_state(&self) -> Self::State {
        panic!()
    }

    fn encode_poll<D: Dumper<Self>>(&self, _: &mut Self::State, dumper: D) -> Result<D, D::Pending> {
        panic!()
    }

    fn encode_blob<W: WriteBlob>(&self, offset: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        self.encode_own_ptr(offset, dst)
    }

    fn encode_own<T: ?Sized + Save<Self>>(own: &Own<T,Self>) -> Result<Self::State, <T as Save<Self>>::State> {
        match own.raw.kind() {
            Kind::Offset(offset) => Ok(offset.persist()),
            Kind::Ptr(ptr) => {
                let value = unsafe { &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), own.metadata) };
                Err(value.init_save_state())
            },
        }
    }

    fn encode_own_value<T, D>(own: &Own<T,Self>, state: &mut T::State, dumper: D) -> Result<(D, Self::State), D::Pending>
        where T: ?Sized + Save<Self>,
              D: Dumper<Self>
    {
        match own.raw.kind() {
            Kind::Ptr(ptr) => {
                let value = unsafe { &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), own.metadata) };
                let (dumper, blob_ptr) = value.save_poll(state, dumper)?;

                if let Some(offset) = Any::downcast_ref(&blob_ptr) {
                    Ok((dumper, *offset))
                } else {
                    unreachable!()
                }
            },
            Kind::Offset(_) => unreachable!(),
        }
    }

    fn encode_own_ptr<W: WriteBlob>(&self, offset: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        offset.encode_blob(&(), dst)
    }
}

impl Decode<Self> for OffsetMut<'_> {
    type Error = OffsetError;

    type ValidateChildren = ();

    fn validate_blob<'q>(blob: Blob<'q, Self, Self>) -> Result<BlobValidator<'q, Self, Self>, Self::Error> {
        let offset_blob = Blob::new(&blob[..], ()).unwrap();
        <Offset as Decode<Offset>>::validate_blob(offset_blob)?;

        Ok(blob.assume_valid(()))
    }

    fn decode_blob<'q>(blob: FullyValidBlob<'q, Self, Self>, _: &impl LoadPtr<Self>) -> Self {
        let inner = <Self as Decode<Self>>::deref_blob(blob).0;
        // TODO: add assertion
        Self(inner)
    }

    fn deref_blob<'q>(blob: FullyValidBlob<'q, Self, Self>) -> &'q Self {
        unsafe { blob.assume_valid() }
    }
}
