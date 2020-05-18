use super::Load;

pub trait BlobValidator<T: ?Sized + Load<Z>, Z> {
    type Ok;
    type Error : From<T::Error>;

    type StructValidator : StructValidator<Z, Ok=Self::Ok, Error=Self::Error>;
    type EnumValidator : EnumValidator<Z, Ok=Self::Ok, Error=Self::Error>;

    fn metadata(&self) -> T::Metadata;

    fn validate_struct(self) -> Self::StructValidator;
    fn validate_enum(self) -> (u8, Self::EnumValidator);

    fn validate_bytes(self, f: impl FnOnce(&[u8]) -> Result<(), T::Error>) -> Result<Self::Ok, Self::Error>;
}

pub trait StructValidator<Z> {
    type Ok;
    type Error;

    fn field<F: Load<Z>>(&mut self, f: impl FnOnce(F::Error) -> Self::Error) -> Result<(), Self::Error>;

    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error>;
}

pub trait EnumValidator<Z> {
    type Ok;
    type Error;

    fn field<F: Load<Z>>(&mut self, f: impl FnOnce(F::Error) -> Self::Error) -> Result<(), Self::Error>;

    /// Asserts that the enum is valid.
    unsafe fn assume_valid(self) -> Result<Self::Ok, Self::Error>;
}
