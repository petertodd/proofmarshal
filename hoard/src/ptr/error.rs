use std::error;

use thiserror::Error;

use super::*;

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct Error<ZoneId: fmt::Debug, ZoneError: error::Error> {
    inner: Box<Inner<ZoneId, ZoneError>>,
}

#[derive(Debug)]
struct Inner<ZoneId: fmt::Debug, ZoneError: error::Error> {
    id: ZoneId,
    kind: ErrorKind<ZoneError>,
}

#[derive(Debug)]
pub enum ErrorKind<ZoneError: error::Error> {
    Zone(ZoneError),
    Decode(Box<dyn error::Error + 'static + Send>),
}

impl<Z, E> Error<Z, E>
where Z: fmt::Debug,
      E: error::Error + 'static + Send,
{
    pub fn from_zone_error(id: Z, err: E) -> Self {
        Self::new(id, ErrorKind::Zone(err))
    }

    pub fn from_decode_error<E2>(id: Z, err: E2) -> Self
        where E2: error::Error + 'static + Send
    {
        Self::new(id, ErrorKind::Decode(Box::new(err)))
    }

    fn new(id: Z, kind: ErrorKind<E>) -> Self {
        let inner = Inner { id, kind };

        Self {
            inner: Box::new(inner),
        }
    }

    pub fn zone_id(&self) -> &Z {
        &self.inner.id
    }

    pub fn kind(&self) -> &ErrorKind<E> {
        &self.inner.kind
    }
}
