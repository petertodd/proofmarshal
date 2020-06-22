//! Implementations on foreign types.

use super::*;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::*;
use crate::zone::*;
use crate::refs::Ref;
use crate::writebytes::WriteBytes;
use crate::scalar::Scalar;
use crate::save::*;

pub mod never;
pub mod scalars;
pub mod array;
pub mod option;
