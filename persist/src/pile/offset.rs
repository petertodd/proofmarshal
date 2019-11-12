use core::cmp;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::num::NonZeroU64;
use core::ptr::{self, NonNull};

use std::alloc::Layout;

use super::*;

use crate::marshal::blob::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset<'p> {
    marker: PhantomData<fn(&'p ()) -> &'p ()>,
    raw: NonZeroU64,
}

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

impl fmt::Pointer for Offset<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.get())
    }
}

impl<'p, Z: Zone> Save<Z> for Offset<'p> {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(8);

    type SavePoll = Offset<'p>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        this.take_sized()
    }
}

impl<'p, Z: Zone> SavePoll<Z> for Offset<'p> {
    type Target = Offset<'p>;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(dbg!(&self.raw.get().to_le_bytes()))?
           .done()
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

impl<'p, Z: Zone> Load<Z> for Offset<'p> {
    type Error = OffsetError;

    type ValidateChildren = ();
    fn validate_blob<'q>(blob: Blob<'q, Self, Z>) -> Result<ValidateBlob<'q, Self, Z>, Self::Error> {
        let mut raw = [0;8];
        raw.copy_from_slice(&blob[..]);
        let raw = u64::from_le_bytes(raw);

        if (raw & 1 != 1) {
            Err(OffsetError(()))
        } else {
            Offset::try_from(raw >> 1)?;

            Ok(blob.assume_valid(()))
        }
    }

    fn decode_blob<'q>(blob: FullyValidBlob<'q, Self, Z>, _: &impl Loader<Z>) -> Self::Owned {
        let mut raw = [0;8];
        raw.copy_from_slice(&blob[..]);
        let raw = u64::from_le_bytes(raw) >> 1;

        Offset::new(raw).unwrap()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OffsetMut<'p>(Offset<'p>);

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

    pub(super) unsafe fn dealloc<T: ?Sized + Pointee>(self, metadata: T::Metadata) {
        let this = ManuallyDrop::new(self);

        match this.kind() {
            Kind::Offset(_) => {},
            Kind::Ptr(ptr) => {
                let r: &mut T = &mut *T::make_fat_ptr_mut(ptr.cast().as_ptr(), metadata);
                let layout = fix_layout(Layout::for_value(r));

                ptr::drop_in_place(r);

                if layout.size() > 0 {
                    std::alloc::dealloc(r as *mut _ as *mut u8, layout);
                }
            }
        }
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
