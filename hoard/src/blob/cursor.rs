use core::any::type_name;
use core::fmt;
use core::mem;

use crate::pointee::Pointee;

use super::*;

pub struct BlobCursor<'a, T: ?Sized + Pointee> {
    blob: Blob<'a, T>,
    offset: usize,
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
    pub fn field_blob<F>(&mut self) -> Blob<F> {
        let size = mem::size_of::<F>();

        let start = self.offset;
        self.offset += size;

        let blob: &'a [u8] = self.blob.clone().into();
        let buf = blob.get(start .. self.offset)
                      .expect("overflow");

        Blob::new(buf, F::make_sized_metadata()).unwrap()
    }
}

impl<'a, T: ?Sized + Pointee> BlobCursor<'a, T>
where T: Validate
{
    fn field<U: Validate, F>(&mut self, f: F) -> Result<ValidBlob<U>, Error<T::Error>>
        where F: FnOnce(U::Error) -> T::Error
    {
        let blob = self.field_blob::<U>();
        match U::validate(BlobCursor::from(blob)) {
            Ok(valid_blob) => Ok(valid_blob),
            Err(Error::Value(e)) => Err(Error::Value(f(e))),
            Err(Error::Padding) => Err(Error::Padding),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    Value(E),

    #[non_exhaustive]
    Padding,
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Error::Value(err)
    }
}

impl<'a, T: ?Sized + Pointee> BlobValidator<T> for BlobCursor<'a, T>
where T: Validate
{
    type Ok = ValidBlob<'a, T>;
    type Error = Error<T::Error>;

    type StructValidator = Self;
    type EnumValidator = Self;

    fn metadata(&self) -> T::Metadata {
        self.blob.metadata
    }

    fn validate_struct(self) -> Self {
        self
    }
    fn validate_enum(self) -> (u8, Self) {
        todo!()
    }

    unsafe fn validate_option<U: Validate, F>(mut self, f: F) -> Result<Self::Ok, Self::Error>
        where F: FnOnce(U::Error) -> T::Error
    {
        assert_eq!(self.offset, 0);

        if self.blob.iter().all(|b| *b == 0) {
            Ok(self.blob.assume_valid())
        } else {
            <Self as StructValidator<T>>::field::<U,_>(&mut self, f)?;
            <Self as StructValidator<T>>::assume_valid(self)
        }
    }

    fn validate_bytes(self, f: impl for<'b> FnOnce(Blob<'b, T>) -> Result<ValidBlob<'b, T>, T::Error>)
        -> Result<Self::Ok, Self::Error>
    {
        assert_eq!(self.offset, 0);
        f(self.blob).map_err(Error::Value)
    }
}

impl<'a, T: ?Sized + Pointee> StructValidator<T> for BlobCursor<'a, T>
where T: Validate
{
    type Ok = ValidBlob<'a, T>;
    type Error = Error<T::Error>;

    fn field<U: Validate, F>(&mut self, f: F) -> Result<ValidBlob<U>, Error<T::Error>>
        where F: FnOnce(U::Error) -> T::Error
    {
        self.field::<U,F>(f)
    }

    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error> {
        debug_assert_eq!(self.blob.len(), self.offset,
                   "Blob not fully validated");
        Ok(self.blob.assume_valid())
    }
}

impl<'a, T: ?Sized + Pointee> EnumValidator<T> for BlobCursor<'a, T>
where T: Validate
{
    type Ok = ValidBlob<'a, T>;
    type Error = Error<T::Error>;

    fn field<U: Validate, F>(&mut self, f: F) -> Result<ValidBlob<U>, Error<T::Error>>
        where F: FnOnce(U::Error) -> T::Error
    {
        self.field::<U,F>(f)
    }

    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod test {
}
