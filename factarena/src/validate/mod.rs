//! In-place validation.

use core::alloc::Layout;
use core::convert;
use core::marker::PhantomData;
use core::ops;

use crate::pointee::Pointee;

pub mod slice;

/// In-place validation.
pub trait Validate<U: ?Sized, A: ?Sized = ()> : Pointee {
    type Error;

    fn validate<'a>(unver: MaybeValid<'a, Self, U>, arena: &A) -> Result<Valid<'a, Self, U>, Self::Error>;
}

/// An unverified `T` reference, in the form of a `U`.
#[derive(Debug)]
pub struct MaybeValid<'a, T: ?Sized + Pointee, U: ?Sized = [u8]> {
    marker: PhantomData<fn(&'a U) -> &'a T>,

    unver: &'a U,
    metadata: T::Metadata,
}

/// A valid `T` reference.
#[derive(Debug)]
pub struct Valid<'a, T: ?Sized + Pointee, U: ?Sized = [u8]> {
    marker: PhantomData<fn(&'a U) -> &'a T>,

    unver: &'a U,
    metadata: T::Metadata,
}





impl<'a, T: ?Sized + Pointee, U: ?Sized> MaybeValid<'a,T,U> {
    /// Creates a new `MaybeValid<'a,T,U>` with the specified metadata.
    #[inline]
    pub fn new(unver: &'a U, metadata: T::Metadata) -> Self {
        assert_eq!(Layout::for_value(unver), T::layout(metadata),
                   "mismatched layouts");

        MaybeValid {
            marker: PhantomData,
            unver, metadata,
        }
    }

    /// Gets the metadata for `T`.
    #[inline]
    pub fn valid_metadata(&self) -> T::Metadata {
        self.metadata
    }

    /// Asserts that this is a valid reference to `T`.
    ///
    /// # Safety
    ///
    /// It is up to the caller to guarantee that the bits of `U` really are valid for `T`.
    #[inline]
    pub unsafe fn assume_init(self) -> Valid<'a,T,U> {
        Valid {
            marker: PhantomData,
            unver: self.unver,
            metadata: self.metadata,
        }
    }
}

impl<'a, T: ?Sized + Pointee> MaybeValid<'a,T> {
    #[inline]
    pub fn validate_struct<'p, A: ?Sized>(self, arena: &'p A) -> StructValidator<'a, 'p, T, A> {
        StructValidator {
            unver: self,
            offset: 0,
            arena,
        }
    }
}

impl<'a, T1: Pointee, U: ?Sized> MaybeValid<'a,T1,U> {
    #[inline]
    pub fn cast<T2>(self) -> MaybeValid<'a,T2,U>
        where T2: Pointee,
    {
        MaybeValid::new(self.unver, T2::make_sized_metadata())
    }
}

impl<'a,T: ?Sized + Pointee, U: ?Sized> ops::Deref for MaybeValid<'a,T,U> {
    type Target = U;

    #[inline]
    fn deref(&self) -> &U {
        &self.unver
    }
}

impl<'a, T: ?Sized + Pointee, U: ?Sized> Valid<'a,T,U> {
    /// Converts this into a `T` reference.
    pub fn to_valid_ref(self) -> &'a T {
        let ptr: *const T = T::make_fat_ptr(self.unver as *const U as *const (),
                                            self.metadata);

        unsafe {
            &*ptr
        }
    }
}

impl<'a, T1: Pointee, U: ?Sized> Valid<'a,T1,U> {
    #[inline]
    pub unsafe fn cast<T2>(self) -> Valid<'a,T2,U>
        where T2: Pointee,
    {
        assert_eq!(Layout::for_value(self.unver), Layout::new::<T2>(),
                   "mismatched layouts");

        Valid {
            marker: PhantomData,
            unver: self.unver,
            metadata: T2::make_sized_metadata(),
        }
    }
}

impl<'a,T: ?Sized + Pointee, U: ?Sized> ops::Deref for Valid<'a,T,U> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        let ptr: *const T = T::make_fat_ptr(self.unver as *const U as *const (),
                                            self.metadata);

        unsafe {
            &*ptr
        }
    }
}

#[derive(Debug)]
pub struct StructValidator<'a, 'p, T: ?Sized + Pointee, A: ?Sized> {
    unver: MaybeValid<'a, T>,
    offset: usize,
    arena: &'p A,
}

impl<'a, 'p, T: ?Sized + Pointee, A: ?Sized> StructValidator<'a, 'p, T, A> {
    #[inline]
    fn field<F: Validate<[u8],A>>(self) -> Result<Self, F::Error> {
        let start = self.offset;
        self.offset += mem::size_of::<F>();

        let buf = self.unver[start .. self.offset];
        let unver_field = MaybeValid::new(&buf, F::make_sized_metadata());

        F::validate(unver_field, &self.arena)?;
        Ok(self)
    }

    #[inline]
    fn finish(self) -> Valid<'a,T> {
        assert_eq!(self.offset, self.unver.len(),
                   "struct not fully validated");

        unsafe {
            self.unver.assume_init()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}
