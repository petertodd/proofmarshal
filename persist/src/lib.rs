//! In-place persistence.

use core::any::type_name;
use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops;
use core::ptr;
use core::slice;

pub mod leint;
pub use self::leint::Le;

pub mod option;

/// A type that can be mem-mapped.
pub unsafe trait Persist : Sized {
    type Error;

    fn validate(maybe: &MaybeValid<Self>) -> Result<&Self, Self::Error>;

    fn validate_bytes(buf: &[u8]) -> Result<&Self, Self::Error> {
        let align = mem::align_of::<Self>();
        if align != 1 {
            panic!("Persist can only be implemented on unaligned types; {} has alignment {}",
                   type_name::<Self>(), align)
        }

        unsafe {
            let maybe: &MaybeValid<Self> = &*(buf.as_ptr().cast());
            Self::validate(maybe)
        }
    }

    fn write_canonical<'b>(&self, dst: UninitBytes<'b, Self>) -> &'b mut [u8];

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut r = vec![0; mem::size_of::<Self>()];

        let dst = UninitBytes::from_bytes(&mut r);
        self.write_canonical(dst);

        r
    }
}

/// A potentially valid `T`.
pub struct MaybeValid<T>(MaybeUninit<T>);

/// Requires `Copy` as we won't run destructors.
impl<T: Copy> Default for MaybeValid<T> {
    #[inline]
    fn default() -> Self {
        MaybeValid(MaybeUninit::zeroed())
    }
}

impl<T> MaybeValid<T> {

    /// Creates a new `&MaybeValid<T>` from a `&MaybeUninit<T>`.
    ///
    /// # Safety
    ///
    /// This is *unsafe* because `uninit` might contain uninitialized bytes.
    #[inline(always)]
    pub unsafe fn from_uninit_ref(uninit_ref: &MaybeUninit<T>) -> &Self {
        &*(uninit_ref as *const _ as *const Self)
    }

    /// Create a `&MaybeValid<T>` from a valid reference.
    #[inline(always)]
    pub fn from_valid_ref(valid_ref: &T) -> &Self {
        unsafe {
            &*(valid_ref as *const _ as *const Self)
        }
    }

    /// Asserts that this is a valid reference.
    #[inline(always)]
    pub unsafe fn assume_valid(&self) -> &T {
        &*(self as *const _ as *const T)
    }

    /// Validate fields in a structure.
    pub fn validate_fields<'a>(&'a self) -> FieldValidator<'a, T> {
        FieldValidator {
            maybe: self,
            offset: 0,
        }
    }
}

impl<T> ops::Deref for MaybeValid<T> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &[u8] {
        // Safe because we require all bytes to be initialized.
        unsafe {
            slice::from_raw_parts(self as *const _ as *const u8,
                                  mem::size_of::<Self>())
        }
    }
}

pub struct FieldValidator<'a, T> {
    maybe: &'a MaybeValid<T>,
    offset: usize,
}

impl<'a, T> FieldValidator<'a, T> {
    /// Validate a field.
    pub fn field<F: Persist>(mut self) -> Result<Self, F::Error> {
        let next_offset = self.offset + mem::size_of::<F>();

        let buf = self.maybe.get(self.offset .. next_offset)
                            .unwrap_or_else(|| panic!("Implementation error"));

        let _ = F::validate_bytes(buf)?;

        self.offset = next_offset;

        Ok(self)
    }

    pub unsafe fn assume_valid(self) -> &'a T {
        if self.offset != mem::size_of::<T>() {
            panic!("Not all fields validated; offset {} but mem::size_of::<{}>() == {}",
                   self.offset, type_name::<T>(), mem::size_of::<T>())
        };

        self.maybe.assume_valid()
    }
}

/// Uninitialized bytes.
pub struct UninitBytes<'a, T> {
    marker: PhantomData<*const T>,
    written: &'a mut [u8],
}

impl<'a, T> UninitBytes<'a, T> {
    #[inline(always)]
    pub fn new(buf: &mut [MaybeUninit<u8>]) -> UninitBytes<'a, T> {
        assert_eq!(buf.len(), mem::size_of::<T>(),
                   "wrong length");

        Self {
            marker: PhantomData,
            written: unsafe {
                slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), 0)
            },
        }
    }

    #[inline(always)]
    pub fn from_bytes(buf: &'a mut impl AsMut<[u8]>) -> UninitBytes<'a, T> {
        let buf = buf.as_mut();
        let buf = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr().cast(),
                                                     buf.len()) };

        Self::new(buf)
    }

    #[inline(always)]
    pub fn remaining(&mut self) -> &mut [MaybeUninit<u8>] {
        let len = mem::size_of::<T>() - self.written.len();

        unsafe {
            let ptr: *mut u8 = self.written.as_mut_ptr()
                                   .offset(self.written.len() as isize);

            slice::from_raw_parts_mut(ptr as *mut MaybeUninit<u8>,
                                      len)
        }
    }

    #[inline(always)]
    pub fn write_bytes(&mut self, buf: impl AsRef<[u8]>) {
        let buf = buf.as_ref();

        let remaining = self.remaining();

        assert!(buf.len() <= remaining.len(), "overflow");

        unsafe {
            ptr::copy(buf.as_ptr(), remaining.as_mut_ptr().cast(),
                      buf.len());

            let new_len = self.written.len() + buf.len();
            self.written = slice::from_raw_parts_mut(self.written.as_mut_ptr(),
                                                    new_len);
        }
    }

    #[inline(always)]
    pub fn write_zeros(&mut self, len: usize) {
        for _ in 0 .. len {
            self.write_bytes([0])
        }
    }

    #[inline(always)]
    pub fn write<F: Persist>(&mut self, field: &F) {
        let field_uninit = self.remaining()
                               .get_mut(0 .. mem::size_of::<F>())
                               .expect("overflow");

        let dst = UninitBytes::<F>::new(field_uninit);
        let written = field.write_canonical(dst);
        assert_eq!(written.len(), mem::size_of::<F>());

        let new_len = self.written.len() + mem::size_of::<F>();

        unsafe {
            self.written = slice::from_raw_parts_mut(self.written.as_mut_ptr(),
                                                    new_len);
        }
    }

    pub fn done(mut self) -> &'a mut [u8] {
        assert_eq!(self.remaining().len(), 0,
                   "uninitialized bytes remaining");

        self.written
    }
}

impl<'a,T> fmt::Debug for UninitBytes<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let remaining_len = mem::size_of::<T>() - self.written.len();
        f.debug_struct(type_name::<Self>())
         .field("written", &self.written)
         .field("remaining_len", &remaining_len)
         .finish()
    }
}

mod impls;

#[cfg(test)]
mod tests {
    use super::*;

    use core::num::NonZeroU8;

    #[test]
    fn test() {
        let u = MaybeValid::<NonZeroU8>::default();

        assert_eq!(&u[..], &[0]);
    }

    #[test]
    fn arrays() {
        let orig = &[true; 10];
        let maybe = MaybeValid::from_valid_ref(orig);
        let r = <[bool;10] as Persist>::validate(maybe).unwrap();
        assert_eq!(orig.as_ptr(), r.as_ptr());

        assert_eq!(orig.canonical_bytes(),
                   [1,1,1,1,1,1,1,1,1,1]);
    }

    #[test]
    fn uninit_bytes() {
        let mut buf = [0;8];

        let mut uninit = UninitBytes::<Le<u64>>::from_bytes(&mut buf);
        uninit.write_bytes(&[1,2]);
        uninit.write_bytes(&[3,4,5,6,7,8]);

        uninit.done();
    }
}
