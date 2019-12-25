use core::fmt;

use super::*;


/// The error when dereferencing a zone pointer fails.
pub enum DerefError<T: ?Sized + Persist, Z: Zone> {
    /// The pointer is invalid.
    ///
    /// If this is returned, the `Zone` couldn't even retrieve a `Blob`, and validation was never
    /// attempted.
    Ptr(PtrError<T, Z>),

    /// The value is invalid.
    ///
    /// The `Zone` could retrieve a `Blob` of the right size for this type. But validation of that
    /// `Blob` failed.
    Value {
        zone: Z,
        ptr: FatPtr<T::Persist, Z::Persist>,
        err: T::Error,
    }
}

pub struct PtrError<T: ?Sized + Persist, Z: Zone> {
    zone: Z,
    ptr: FatPtr<T::Persist, Z::Persist>,
    err: Z::Error,
}

impl<T: ?Sized + Persist, Z: Zone> From<PtrError<T, Z>> for !
where Z::Error: Into<!>
{
    fn from(err: PtrError<T,Z>) -> ! {
        err.err.into()
    }
}

impl<T: ?Sized + Persist, Z: Zone> From<PtrError<T,Z>> for DerefError<T,Z> {
    fn from(err: PtrError<T, Z>) -> Self {
        DerefError::Ptr(err)
    }
}

impl<T: ?Sized + Persist, Z: Zone> From<DerefError<T,Z>> for !
where Z::Error: Into<!>,
{
    fn from(err: DerefError<T,Z>) -> ! {
        match err {
            DerefError::Ptr(err) => err.into(),
            DerefError::Value { err, .. } => {
                panic!("{:?}", err) // FIXME
            }
        }
    }
}

impl<T: ?Sized + Persist, Z: Zone> PtrError<T,Z> {
    pub fn new(zone: &Z, ptr: FatPtr<T::Persist, Z::Persist>, err: Z::Error) -> Self {
        Self {
            zone: zone.duplicate(),
            ptr: ptr,
            err,
        }
    }
}

// Debug impls
impl<T: ?Sized + Persist, Z: Zone> fmt::Debug for PtrError<T, Z>
where Z: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("zone", &self.zone)
            .field("ptr", &self.ptr)
            .field("err", &self.err)
            .finish()
    }
}

impl<T: ?Sized + Persist, Z: Zone> fmt::Debug for DerefError<T,Z>
where Z: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DerefError::Ptr(err) =>
                f.debug_tuple("Ptr")
                 .field(&err)
                 .finish(),
            DerefError::Value { zone, ptr, err } =>
                f.debug_struct("Value")
                 .field("zone", zone)
                 .field("ptr", ptr)
                 .field("err", err)
                 .finish(),
        }
    }
}
