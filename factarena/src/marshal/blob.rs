use core::marker::PhantomData;
use core::ops;
use core::any::type_name;

use crate::pointee::Pointee;

/// A byte blob of the same size as `T`.
#[derive(Debug)]
pub struct BlobRef<'a, T: ?Sized + Pointee> {
    marker: PhantomData<fn() -> &'a T>,

    unver: *const u8,
    metadata: T::Metadata,
}

impl<'a,T: ?Sized + Pointee> ops::Deref for BlobRef<'a,T> {
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
pub struct UnvalidatedStruct<'a, 'p, T: ?Sized + Pointee, A: Arena, V: VerifyPtr<A>> {
    marker: PhantomData<*const A>,
    unver: Unvalidated<'a, T>,
    offset: usize,
    arena: &'p V,
}
*/

impl<'a, T: ?Sized + Pointee> BlobRef<'a,T> {
    /*
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
    */

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
    pub unsafe fn assume_valid(self) -> &'a T {
        assert_eq!(T::layout(self.metadata).align(), 1,
                   "Type {} needs alignment",
                   type_name::<T>());

        let ptr: *const T = T::make_fat_ptr(self.unver as *const (),
                                            self.metadata);

        &*ptr
    }
}

    /*
    #[inline]
    pub fn cast_unsized<T2>(self, new_metadata: T2::Metadata) -> Unvalidated<'a, T2>
        where T2: ?Sized + Pointee,
    {
        assert_eq!(T::layout(self.metadata), T2::layout(new_metadata),
                   "Layouts of {} and {} are incompatible",
                   type_name::<T>(), type_name::<T2>());

        Unvalidated {
            marker: PhantomData,
            unver: self.unver,
            metadata: new_metadata,
        }
    }

    #[inline]
    pub fn cast<T2>(self) -> Unvalidated<'a, T2>
        where T2: Pointee,
    {
        self.cast_unsized(T2::make_sized_metadata())
    }

    #[inline]
    pub fn verify_struct<'p, A, V>(self, arena: &'p V) -> UnvalidatedStruct<'a, 'p, T, A, V>
        where A: Arena,
              V: VerifyPtr<A>
    {
        UnvalidatedStruct {
            marker: PhantomData,
            unver: self,
            offset: 0,
            arena,
        }
    }
}

impl<'a,'p,T: ?Sized + Pointee, A: Arena, V: VerifyPtr<A>> UnvalidatedStruct<'a,'p,T,A,V> {
    pub fn field<F: Persist<A>>(mut self) -> Result<Self, F::Error> {
        let start = self.offset;
        self.offset += mem::size_of::<F>();

        let field_buf = &self.unver[start .. self.offset];
        let unver_field = Unvalidated::<F>::new(&field_buf);

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

*/
