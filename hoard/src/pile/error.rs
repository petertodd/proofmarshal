use std::error::Error;
use std::fmt::Debug;

use thiserror::Error;

use crate::pointee::Pointee;

use super::*;

#[derive(Debug, Error)]
#[error("fixme")]
pub enum GetBlobError<LayoutError: Debug> {
    OutOfRange,
    Layout(LayoutError),
}

#[derive(Debug, Error)]
#[error("fixme")]
pub enum GetValidBlobError<LayoutError: Debug, ValidateError: Debug> {
    Blob(GetBlobError<LayoutError>),
    Validate(ValidateError),
}

impl<L: Error, V: Error> From<GetBlobError<L>> for GetValidBlobError<L, V> {
    fn from(err: GetBlobError<L>) -> Self {
        Self::Blob(err)
    }
}

/*
#[derive(Debug, Error)]
#[error("FIXME")]
pub struct Error<
    P: Debug,
    M: ?Sized + Debug,
    L: Debug,
    V: Debug,
>
{
    pile: P,
    offset: Offset,
    kind: ErrorKind<L, V>,
    metadata: M,
}

#[derive(Debug)]
pub enum ErrorKind<L, V> {
    Layout(L),
    OutOfRange(usize),
    Validate(V),
}

impl<P, M, L, V> Error<P, M, L, V>
where P: Debug, M: Debug, L: Debug, V: Debug
{
    pub fn new<Z>(pile: P, offset: &Offset<Z>, metadata: M, kind: ErrorKind<L, V>) -> Self {
        Self {
            pile,
            offset: offset.to_static(),
            metadata: metadata,
            kind,
        }
    }
}

/*
    pub fn offset(&self) -> &Offset {
        &self.offset
    }

    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    pub fn kind(&self) -> &ErrorKind<L, V> {
        &self.kind
    }
}
*/

*/
