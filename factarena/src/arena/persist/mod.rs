/// In-place persistent data.

use std::io;

use core::any::type_name;
use core::borrow::Borrow;
use core::marker::PhantomData;
use core::fmt;
use core::ops;
use core::mem;

use crate::pointee::Pointee;
use crate::util::nonzero::NonZero;

use super::*;

pub mod primitive;
pub mod leint;

pub mod option;
pub mod slice;
pub mod own;

use super::{Arena, Type};

/// Types whose values can be safely mem-mapped.
pub unsafe trait Persist<A: Arena = !> : Pointee {
    type Error : fmt::Debug;

    fn verify<'a>(unver: Unverified<'a, Self>, arena: &impl VerifyPtr<A>)
        -> Result<&'a Self, Self::Error>;

    fn write_canonical_bytes<W: io::Write>(&self, mut w: W) -> io::Result<W> {
        unsafe {
            let buf = std::slice::from_raw_parts(self as *const Self as *const u8,
                                                 mem::size_of_val(self));

            w.write_all(buf)?;
            Ok(w)
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let r = vec![];
        self.write_canonical_bytes(r).unwrap()
    }
}

pub trait VerifyPtr<A: Arena> {
    fn verify_ptr<T: ?Sized + Type<A>>(&self, ptr: &A::Ptr, metadata: T::Metadata) -> Result<(), T::Error>;
}

impl VerifyPtr<!> for () {
    fn verify_ptr<T: ?Sized + Type<!>>(&self, ptr: &!, _: T::Metadata) -> Result<(), T::Error> {
        match *ptr {}
    }
}

/// An unverified `T` reference.
#[derive(Debug)]
pub struct Unverified<'a, T: ?Sized + Pointee> {
    marker: PhantomData<fn() -> &'a T>,

    unver: *const u8,
    metadata: T::Metadata,
}

pub struct UnverifiedStruct<'a, 'p, T: ?Sized + Pointee, A: Arena, V: VerifyPtr<A>> {
    marker: PhantomData<*const A>,
    unver: Unverified<'a, T>,
    offset: usize,
    arena: &'p V,
}

impl<'a, T: ?Sized + Pointee> Unverified<'a,T> {
    #[inline]
    pub fn new(unver: &'a impl Borrow<[u8]>) -> Self
        where T: Sized,
    {
        Self::new_unsized(unver, T::make_sized_metadata())
    }

    /// Creates a new `MaybeValid<'a,T,U>` with the specified metadata.
    #[inline]
    pub fn new_unsized(unver: &'a impl Borrow<[u8]>, metadata: T::Metadata) -> Self {

        let unver_bytes = unver.borrow();
        assert_eq!(T::layout(metadata).size(), unver_bytes.len(),
                   "wrong length");

        Self {
            marker: PhantomData,
            unver: unver_bytes.as_ptr() as *const u8,
            metadata,
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
    pub unsafe fn assume_init(self) -> &'a T {
        assert_eq!(T::layout(self.metadata).align(), 1,
                   "Type {} needs alignment",
                   type_name::<T>());

        let ptr: *const T = T::make_fat_ptr(self.unver as *const (),
                                            self.metadata);

        &*ptr
    }

    #[inline]
    pub fn cast_unsized<T2>(self, new_metadata: T2::Metadata) -> Unverified<'a, T2>
        where T2: ?Sized + Pointee,
    {
        assert_eq!(T::layout(self.metadata), T2::layout(new_metadata),
                   "Layouts of {} and {} are incompatible",
                   type_name::<T>(), type_name::<T2>());

        Unverified {
            marker: PhantomData,
            unver: self.unver,
            metadata: new_metadata,
        }
    }

    #[inline]
    pub fn cast<T2>(self) -> Unverified<'a, T2>
        where T2: Pointee,
    {
        self.cast_unsized(T2::make_sized_metadata())
    }

    #[inline]
    pub fn verify_struct<'p, A, V>(self, arena: &'p V) -> UnverifiedStruct<'a, 'p, T, A, V>
        where A: Arena,
              V: VerifyPtr<A>
    {
        UnverifiedStruct {
            marker: PhantomData,
            unver: self,
            offset: 0,
            arena,
        }
    }
}

impl<'a,'p,T: ?Sized + Pointee, A: Arena, V: VerifyPtr<A>> UnverifiedStruct<'a,'p,T,A,V> {
    pub fn field<F: Persist<A>>(mut self) -> Result<Self, F::Error> {
        let start = self.offset;
        self.offset += mem::size_of::<F>();

        let field_buf = &self.unver[start .. self.offset];
        let unver_field = Unverified::<F>::new(&field_buf);

        F::verify(unver_field, self.arena)?;
        Ok(self)
    }

    pub fn finish<E>(self) -> Result<&'a T, E> {
        assert_eq!(self.offset, self.unver.len(),
                   "struct verification incomplete");

        unsafe {
            Ok(self.unver.assume_init())
        }
    }
}

impl<'a,T: ?Sized + Pointee> ops::Deref for Unverified<'a,T> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        let len = T::layout(self.metadata).size();

        unsafe {
            core::slice::from_raw_parts(self.unver, len)
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_new() {
        let b = [42];

        let unver = Unverified::<u8>::new(&b);

        assert_eq!(&*unver, &[42]);

        let valid = unsafe { unver.assume_init() };
        let b_ref = valid.to_valid_ref();
        assert_eq!(*b_ref, 42);
    }

    #[test]
    #[should_panic(expected = "wrong length")]
    fn unverified_new_wrong_size() {
        let b = [42,43];

        let _ = Unverified::<u8>::new(&b);
    }

    #[test]
    #[should_panic(expected = "Type u16 needs alignment")]
    fn aligned_type() {
        let b = [12,34];
        let unver = Unverified::<u16>::new(&b);

        let _ = unsafe { unver.assume_init() };
    }
}

/*

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
}*/*/
