//! Blob validation.

use core::any::type_name;
use core::fmt;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::{self, Range};
use core::ptr;
use core::slice;

use super::*;

use crate::marshal::primitive::Primitive;

pub(super) struct BlobCursor<'a, T: ?Sized + Pointee> {
    pub(super) blob: Blob<'a, T>,
    pub(super) offset: usize,
}

impl<'a, T: ?Sized + Pointee> From<Blob<'a, T>> for BlobCursor<'a, T> {
    fn from(blob: Blob<'a, T>) -> Self {
        Self { blob, offset: 0 }
    }
}

impl<T: ?Sized + Pointee> fmt::Debug for BlobCursor<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("blob", &self.blob)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a, T: ?Sized + Pointee> BlobCursor<'a, T> {
    pub fn field_blob<F>(&mut self) -> Blob<'a, F> {
        let size = mem::size_of::<F>();

        let start = self.offset;
        self.offset += size;

        let blob: &'a [u8] = self.blob.clone().into();
        let buf = blob.get(start .. self.offset)
                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }

/*
    fn validate_field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        let blob = self.field_blob::<F>();
        let validator = F::validate_blob(blob)?;
        Ok(validator.state)
    }
*/

    fn primitive_field<F: 'a + Primitive>(&mut self) -> Result<ValidBlob<'a, F>, F::Error> {
        let blob = self.field_blob::<F>();
        F::validate_blob(blob)
    }
}

/// Struct validator.
pub struct StructValidator<'a, T: ?Sized + Pointee>(pub(super) BlobCursor<'a,T>);

impl<'a, T: ?Sized + Pointee> StructValidator<'a, T> {
    pub fn field_blob<F>(&mut self) -> Blob<'a, F> {
        self.0.field_blob::<F>()
    }

/*
    pub fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }
*/

    pub fn primitive_field<F: 'a + Primitive>(&mut self) -> Result<ValidBlob<'a, F>, F::Error> {
        self.0.primitive_field::<F>()
    }

    pub unsafe fn done(self) -> ValidBlob<'a, T> {
        assert_eq!(self.0.offset, self.0.blob.len(),
                   "not fully validated");

        self.0.blob.assume_valid()
    }
}

/// Struct validator for `Primitive` structs.
pub struct PrimitiveStructValidator<'a, T: ?Sized + Pointee>(pub(super) BlobCursor<'a,T>);

impl<'a, T: ?Sized + Pointee> PrimitiveStructValidator<'a, T> {
    pub fn field_blob<F>(&mut self) -> Blob<'a, F> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Primitive>(&mut self) -> Result<ValidBlob<'a, F>, F::Error> {
        self.0.primitive_field::<F>()
    }

    pub unsafe fn done(self) -> ValidBlob<'a, T> {
        assert_eq!(self.0.offset, self.0.blob.len(),
                   "not fully validated");

        self.0.blob.assume_valid()
    }
}

/// Enum variant validator.
pub struct VariantValidator<'a, T: ?Sized + Pointee>(pub(super) BlobCursor<'a,T>);

impl<'a, T: ?Sized + Pointee> StructValidator<'a, T> {
}

/*
impl<'a, T: ?Sized + Load<P>, P: Ptr> VariantValidator<'a, T, P> {
    pub fn field_blob<F: Decode<P>>(&mut self) -> Blob<'a, F, P> {
        self.0.field_blob::<F>()
    }

    pub fn field<F: 'a + Decode<P>>(&mut self) -> Result<F::ValidateChildren, F::Error> {
        self.0.validate_field::<F>()
    }

    pub fn done(self, state: T::ValidateChildren) -> Result<BlobValidator<'a, T, P>, PaddingError> {
        if self.0.blob[self.0.offset .. ].iter().all(|b| *b == 0) {
            Ok(self.0.blob.assume_valid(state))
        } else {
            Err(PaddingError(()))
        }
    }
}
*/

#[cfg(test)]
mod test {
}
