use std::fmt;

use thiserror::Error;

use crate::commit::Digest;

#[derive(Error)]
#[non_exhaustive]
pub enum UnpruneError<T, ZoneError: std::error::Error> {
    #[error("evidence missing")]
    #[non_exhaustive]
    Missing {
        digest: Digest<T>,
    },

    #[error("zone error")]
    #[non_exhaustive]
    Zone {
        digest: Digest<T>,
        err: ZoneError,
    },
}

impl<T, Z: std::error::Error> fmt::Debug for UnpruneError<T, Z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UnpruneError::Missing { digest } => {
                f.debug_struct("Missing")
                    .field("digest", digest)
                    .finish()
            },
            UnpruneError::Zone { digest, err } => {
                f.debug_struct("Zone")
                    .field("digest", digest)
                    .field("err", err)
                    .finish()
            }
        }
    }
}
