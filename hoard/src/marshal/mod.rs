//! In-place data marshalling.
//!
//!

use owned::Owned;

use crate::pointee::Pointee;
use crate::zone::Zone;

pub mod blob;
use self::blob::Blob;

pub trait Load<Z> : Pointee + Owned {
    type Error : 'static;

    type ChildValidator;

    fn validate<B>(blob: B) -> Result<B::Ok, B::Error>
        where B: blob::validate::BlobValidator<Self, Z>;

    fn validate_children(&self) -> Self::ChildValidator;
}

pub trait Validate : Pointee {
    type Error : 'static;

    fn validate<B>(blob: B) -> Result<B::Ok, B::Error>
        where B: blob::validate::BlobValidator<Self>;
}
