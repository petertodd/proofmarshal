//! Implementations on foreign types.

use static_assertions::assert_impl_all;

use crate::pointee::Pointee;

use crate::marshal::blob;
use crate::marshal::decode::*;
use crate::marshal::PtrValidator;

pub mod scalar;
pub mod array;
pub mod option;
