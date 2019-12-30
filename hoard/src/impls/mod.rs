//! Implementations on foreign types.

use static_assertions::assert_impl_all;

use crate::pointee::Pointee;

use crate::marshal::blob::*;
use crate::marshal::decode::*;
use crate::marshal::encode::*;
use crate::marshal::{PtrValidator, Dumper};

pub mod scalar;
pub mod option;
//pub mod array;
