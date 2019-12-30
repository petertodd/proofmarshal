use core::any::Any;
use core::fmt;
use core::marker::PhantomData;

use super::*;

use crate::marshal::load::PersistPointee;

/// Returned when attempting to dereference a `Zone` pointer fails.
pub enum DerefError<T: ?Sized + PersistPointee, Z: Zone> {
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
        ptr: Z::ErrorPtr,
        metadata: T::Metadata,
        err: <T as PersistPointee>::Error,
    },
}

/// Returned when a pointer is invalid.
pub enum PtrError<T: ?Sized + Pointee, Z: Zone> {
    Zone {
        ptr: Z::ErrorPtr,
        metadata: T::Metadata,
    },
    Layout {
        ptr: Z::ErrorPtr,
        metadata: T::Metadata,
        err: T::LayoutError,
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> From<PtrError<T,Z>> for DerefError<T,Z> {
    fn from(err: PtrError<T, Z>) -> Self {
        DerefError::Ptr(err)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> From<PtrError<T, Z>> for !
where Z::Error: Into<!>
{
    fn from(err: PtrError<T,Z>) -> ! {
        unreachable!()
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> From<DerefError<T, Z>> for !
where Z::Error: Into<!>
{
    fn from(err: DerefError<T,Z>) -> ! {
        unreachable!()
    }
}

pub type Maybe<T, Z> = Result<T, <Z as Zone>::Error>;

pub trait ZoneError : Sized + fmt::Debug {
    fn into_dyn(self) -> Box<dyn ZoneErrorDyn>;
}

impl ZoneError for ! {
    fn into_dyn(self) -> Box<dyn ZoneErrorDyn> {
        match self {}
    }
}

pub trait FromDerefError<Z: Zone> : ZoneError {
    fn from_deref_error<T: ?Sized + PersistPointee>(err: DerefError<T, Z>) -> Self;
}

impl<Z: Zone> FromDerefError<Z> for !
where Z::Error: Into<!>
{
    fn from_deref_error<T: ?Sized + PersistPointee>(err: DerefError<T, Z>) -> Self {
        Into::<!>::into(err)
    }
}

pub trait ZoneErrorDyn : 'static + Any + fmt::Debug {
}

impl<E: 'static + Any + fmt::Debug> ZoneErrorDyn for E {
}

impl<E: ZoneError> From<E> for Box<dyn ZoneErrorDyn> {
    fn from(err: E) -> Self {
        err.into_dyn()
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> From<DerefError<T,Z>> for Box<dyn ZoneErrorDyn> {
    fn from(err: DerefError<T,Z>) -> Self {
        Z::Error::from_deref_error(err).into()
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> From<PtrError<T,Z>> for Box<dyn ZoneErrorDyn> {
    fn from(err: PtrError<T,Z>) -> Self {
        DerefError::from(err).into()
    }
}


// Debug impls
impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for PtrError<T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        /*
        match self {
            PtrError::Zone { zone, ptr, err } => {
                f.debug_struct("Zone")
                 .field("zone", zone)
                 .field("ptr", ptr)
                 .field("err", err)
                 .finish()
            },
            PtrError::Layout { zone, ptr, err } => {
                f.debug_struct("Layout")
                 .field("zone", zone)
                 .field("ptr", ptr)
                 .field("err", err)
                 .finish()
            },
        }
        */ todo!()
    }
}

impl<T: ?Sized + PersistPointee, Z: Zone> fmt::Debug for DerefError<T, Z>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        /*
        match self {
            DerefError::Ptr(e) => {
                f.debug_tuple("Ptr")
                 .field(e)
                 .finish()
            },
            DerefError::Value { zone, ptr, err } => {
                f.debug_struct("Value")
                 .field("zone", zone)
                 .field("ptr", ptr)
                 .field("err", err)
                 .finish()
            },
        }*/ todo!()
    }
}
