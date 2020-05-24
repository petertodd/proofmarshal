use super::*;

use std::marker::PhantomData;

use crate::blob::ValidateBlobPtr;

pub struct TryGetError<Z, P, T: ?Sized,
    Metadata = <T as Pointee>::Metadata,
    LayoutError = <T as Pointee>::LayoutError,
    LoadError = <T as ValidateBlobPtr>::Error,
    ZoneError = <Z as TryGet<P>>::Error,
> {
    marker: PhantomData<*const T>,
    zone: Z,
    ptr: P,
    metadata: Metadata,

    kind: ErrorKind<LayoutError, LoadError, ZoneError>,
}

pub enum ErrorKind<LayoutError, LoadError, ZoneError> {
    Layout(LayoutError),
    Load(LoadError),
    Zone(ZoneError),
}
