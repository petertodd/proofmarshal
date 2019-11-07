use core::cmp;
use core::convert::TryFrom;
use core::fmt;
use core::marker::PhantomData;
use core::num::NonZeroU64;
use core::ptr::NonNull;

use persist::Le;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    raw: Le<NonZeroU64>
}

impl Offset<'_> {
    const MAX: u64 = u64::max_value() >> 2;

    #[inline]
    pub unsafe fn new_unchecked(words: u64) -> Self {
        let words = (words << 1) | 1;
        Self {
            marker: PhantomData,
            raw: NonZeroU64::new_unchecked(words).into(),
        }
    }

    #[inline]
    pub fn get(self) -> u64 {
        self.raw.get().get() >> 1
    }
}

pub union OffsetMut<'h> {
    offset: Offset<'h>,
    ptr: NonNull<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind<'h> {
    Offset(Offset<'h>),
    Ptr(NonNull<()>),
}

impl<'h> OffsetMut<'h> {
    pub fn from_offset(offset: Offset<'h>) -> Self {
        Self { offset }
    }

    pub unsafe fn from_ptr(ptr: NonNull<()>) -> Self {
        Self { ptr }
    }

    pub fn kind(&self) -> Kind<'h> {
        unsafe {
            if self.ptr.as_ptr() as usize & 1 == 0 {
                Kind::Ptr(self.ptr)
            } else {
                Kind::Offset(self.offset)
            }
        }
    }
}

impl fmt::Debug for OffsetMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl cmp::PartialEq for OffsetMut<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind()
    }
}
impl cmp::Eq for OffsetMut<'_> {}

impl cmp::PartialOrd for OffsetMut<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.kind().partial_cmp(&other.kind())
    }
}
impl cmp::Ord for OffsetMut<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.kind().cmp(&other.kind())
    }
}

/*
#[derive(Debug, PartialEq, Eq)]
pub struct OffsetError(u64);


impl From<Offset> for u64 {
    #[inline(always)]
    fn from(offset: Offset) -> u64 {
        offset.get()
    }
}

impl TryFrom<u64> for Offset {
    type Error = OffsetError;

    #[inline]
    fn try_from(words: u64) -> Result<Self, OffsetError> {
        match words {
            x if 0 < x && x <= Offset::MAX => Ok(unsafe { Offset::new_unchecked(x) }),
            x => Err(OffsetError(x)),
        }
    }
}

impl TryFrom<usize> for Offset {
    type Error = OffsetError;

    #[inline]
    fn try_from(words: usize) -> Result<Self, OffsetError> {
        Self::try_from(words as u64)
    }
}

/// Raw pointer to a record in a snapshot.
pub union Ptr<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    offset: Offset,
    ptr: NonNull<()>,
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_tryfrom_u64() {
        assert_eq!(Offset::try_from(0u64),
                   Err(OffsetError(0)));

        assert_eq!(Offset::try_from((1u64 << 62)),
                   Err(OffsetError(1u64 << 62)));

        assert_eq!(Offset::try_from(1u64).unwrap().get(),
                   1);
        assert_eq!(Offset::try_from(0x3fff_ffff__ffff_ffff_u64).unwrap().get(),
                   0x3fff_ffff__ffff_ffff_u64);
    }

    #[test]
    fn ptr_from_offset() {
        unsafe {
            let offset = Offset::try_from(1u64).unwrap();
            let ptr = Ptr::from_offset(offset);

            assert_eq!(ptr.kind(), Kind::Offset(offset));
        }
    }

    #[test]
    fn ptr_from_ptr() {
        unsafe {
            let nonnull = NonNull::<u16>::dangling().cast();
            let ptr = Ptr::from_ptr(nonnull);
            assert_eq!(ptr.kind(), Kind::Ptr(nonnull));
        }
    }
}

/*
    pub fn get(self) -> usize {
        self.offset.get().get() as usize
    }
}

/// Read-only snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot<'h> {
    marker: PhantomData<fn(&'h ()) -> &'h ()>,
    pub(crate) mapping: Arc<Mmap>,
}

impl<'h> Snapshot<'h> {
    pub(crate) fn from_mapping(mapping: Arc<Mmap>) -> Self {
        Self {
            marker: PhantomData,
            mapping,
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.mapping[mem::size_of::<Header>() - mem::size_of::<Word>() .. ]
    }
}
*/
*/
