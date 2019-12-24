use super::*;

/// The error when dereferencing a zone pointer fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PtrError<ValueError, ZoneError> {
    /// The pointer is invalid.
    ///
    /// If this is returned, the `Zone` couldn't even retrieve a `Blob`, and validation was never
    /// attempted.
    Ptr(ZoneError),

    /// The value is invalid.
    ///
    /// The `Zone` could retrieve a `Blob` of the right size for this type. But validation of that
    /// `Blob` failed.
    Value(ValueError),
}

/// Result returned by zone methods that dereference persistent pointers.
pub type PtrResult<R, T, Z> = core::result::Result<R, PtrError<<T as Load<Z>>::Error, <Z as Zone>::Error>>;
